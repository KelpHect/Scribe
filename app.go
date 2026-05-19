package main

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	stdRuntime "runtime"
	"runtime/debug"
	"sort"
	"strings"
	"sync"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"

	"Scribe/internal/addon"
	"Scribe/internal/esoui"
	"Scribe/internal/scanner"
	"Scribe/internal/settings"

	"time"

	"github.com/google/uuid"

	"gorm.io/gorm"
)

type App struct {
	ctx     context.Context
	scanner *scanner.Scanner
	perfMu  sync.RWMutex

	version   string
	commit    string
	buildDate string

	startedAt       time.Time
	domReadyAt      time.Time
	frontendReadyAt time.Time
	remoteReadyAt   time.Time
	detailFetches   map[string]int
	lastDetailUID   string
	lastDetailAt    time.Time
	remoteRefreshes int
	lastRefreshAt   time.Time
	lastRefreshMS   int64
	lastRefreshErr  string
	refreshInFlight bool
	refreshStarted  time.Time

	scanMu             sync.RWMutex
	cachedStateReadyAt time.Time
	scanStartedAt      time.Time
	scanReadyAt        time.Time
	scanInFlight       bool
	lastScanErr        string

	frontendReadyOff func()
	perfCaptureOff   func()

	db *gorm.DB

	persistenceMu     sync.RWMutex
	persistenceStatus string
	persistenceError  string

	esoClient        *esoui.Client
	esoCache         *esoui.Cache
	remoteMu         sync.RWMutex
	remoteList       []esoui.RemoteAddon
	remoteCategories []esoui.Category

	downloads *esoui.DownloadManager

	settingsMgr *settings.Manager

	tempCleanupMu       sync.RWMutex
	tempCleanupRemoved  int
	tempCleanupRetained int
	tempCleanupError    string

	shutdownCtx    context.Context
	shutdownCancel context.CancelFunc
	initWg         sync.WaitGroup
	refreshWg      sync.WaitGroup

	initDone chan struct{}
}

func NewApp() *App {
	return &App{
		initDone:      make(chan struct{}),
		startedAt:     time.Now(),
		detailFetches: make(map[string]int),
	}
}

type DiagnosticsCount struct {
	Name  string `json:"name"`
	Count int    `json:"count"`
}

type DiagnosticsSnapshot struct {
	StartupMS           int64              `json:"startupMs"`
	UptimeMS            int64              `json:"uptimeMs"`
	DOMReadyMS          int64              `json:"domReadyMs"`
	FrontendReadyMS     int64              `json:"frontendReadyMs"`
	RemoteReadyMS       int64              `json:"remoteReadyMs"`
	HeapAllocMB         uint64             `json:"heapAllocMb"`
	SysMB               uint64             `json:"sysMb"`
	Goroutines          int                `json:"goroutines"`
	NumGC               uint32             `json:"numGc"`
	RemoteAddons        int                `json:"remoteAddons"`
	RemoteCategories    int                `json:"remoteCategories"`
	InstalledAddons     int                `json:"installedAddons"`
	RemoteCacheStale    bool               `json:"remoteCacheStale"`
	DetailRequests      int                `json:"detailRequests"`
	DetailUniqueUIDs    int                `json:"detailUniqueUids"`
	LastDetailUID       string             `json:"lastDetailUid"`
	LastDetailAt        string             `json:"lastDetailAt"`
	DetailTop           []DiagnosticsCount `json:"detailTop"`
	RemoteRefreshCount  int                `json:"remoteRefreshCount"`
	LastRemoteRefreshAt string             `json:"lastRemoteRefreshAt"`
	LastRemoteRefreshMS int64              `json:"lastRemoteRefreshMs"`
	HeapInUseMB         uint64             `json:"heapInUseMb"`
	StackInUseMB        uint64             `json:"stackInUseMb"`
	TotalAllocMB        uint64             `json:"totalAllocMb"`
	MemoryBudgetOK      bool               `json:"memoryBudgetOk"`
	StartupBudgetOK     bool               `json:"startupBudgetOk"`
	PersistenceStatus   string             `json:"persistenceStatus"`
	PersistenceError    string             `json:"persistenceError"`
	CachedStateReadyMS  int64              `json:"cachedStateReadyMs"`
	ScanStartedMS       int64              `json:"scanStartedMs"`
	ScanReadyMS         int64              `json:"scanReadyMs"`
	ScanInFlight        bool               `json:"scanInFlight"`
	LastScanError       string             `json:"lastScanError"`
	TempCleanupRemoved  int                `json:"tempCleanupRemoved"`
	TempCleanupRetained int                `json:"tempCleanupRetained"`
	TempCleanupError    string             `json:"tempCleanupError"`
}

type RemoteCatalogStatus struct {
	HasData          bool   `json:"hasData"`
	CacheStale       bool   `json:"cacheStale"`
	LastRefreshError string `json:"lastRefreshError"`
	RefreshInFlight  bool   `json:"refreshInFlight"`
	RefreshStartedAt string `json:"refreshStartedAt"`
}

func (a *App) shutdown(ctx context.Context) {
	if a.shutdownCancel != nil {
		a.shutdownCancel()
	}

	if a.frontendReadyOff != nil {
		a.frontendReadyOff()
	}
	if a.perfCaptureOff != nil {
		a.perfCaptureOff()
	}
	if a.downloads != nil {
		a.downloads.Shutdown()
	}

	a.initWg.Wait()
	a.refreshWg.Wait()

	if a.esoClient != nil {
		a.esoClient.CloseIdleConnections()
	}
	if a.db != nil {
		if sqlDB, err := a.db.DB(); err == nil {
			_ = sqlDB.Close()
		}
	}
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx

	a.downloads = esoui.NewDownloadManager(3, func(eventName string, data any) {
		wailsRuntime.EventsEmit(ctx, eventName, data)
	})
	a.downloads.OnComplete = func(uid, md5Hash string) {
		if a.db != nil {
			_ = esoui.SaveInstallMD5(a.db, uid, md5Hash)
		}
	}
	a.frontendReadyOff = wailsRuntime.EventsOn(ctx, "perf:frontend-ready", func(optionalData ...interface{}) {
		a.markFrontendReady()
	})
	a.perfCaptureOff = wailsRuntime.EventsOn(ctx, "perf:capture", func(optionalData ...interface{}) {
		label := "capture"
		if len(optionalData) > 0 {
			if s, ok := optionalData[0].(string); ok && s != "" {
				label = s
			}
		}
		a.logPerformanceSnapshot(label)
	})

	detectedPath := scanner.DetectAddonPath()
	a.scanner = scanner.New(detectedPath)

	if db, err := esoui.OpenAppDB(); err == nil {
		a.db = db
		a.setPersistenceStatus("ok", "")

		a.settingsMgr = settings.NewManager(db)
		a.configureScannerCache()

		if detectedPath == "" {
			if s, err2 := a.settingsMgr.GetSettings(); err2 == nil && s.AddonPath != "" {
				a.scanner = scanner.New(s.AddonPath)
				a.configureScannerCache()
			}
		}
	} else {
		a.setPersistenceStatus("degraded", privacySafePersistenceError(err))
		wailsRuntime.LogErrorf(a.ctx, "[db] settings/cache database unavailable: %s", a.getPersistenceError())
	}

	a.markCachedStateReady()
	a.shutdownCtx, a.shutdownCancel = context.WithCancel(context.Background())
	maybeStartPprof(a.shutdownCtx)
	a.cleanStaleInstallArtifacts()
	a.startBackgroundAddonScan("startup")
	a.initWg.Add(1)
	go a.initESOUI()
}

func (a *App) cleanStaleInstallArtifacts() {
	if a.scanner == nil {
		return
	}
	addonPath := a.scanner.GetAddonPath()
	if addonPath == "" {
		return
	}

	report := esoui.CleanStaleInstallArtifacts(addonPath, time.Hour)
	a.tempCleanupMu.Lock()
	a.tempCleanupRemoved = report.RemovedCount()
	a.tempCleanupRetained = report.RetainedCount()
	a.tempCleanupError = privacySafeInstallCleanupError(report.Error())
	a.tempCleanupMu.Unlock()

	if report.RemovedCount() > 0 || report.RetainedCount() > 0 || report.Error() != "" {
		wailsRuntime.LogInfof(
			a.ctx,
			"[install-cleanup] removed=%d retained=%d errors=%d",
			report.RemovedCount(),
			report.RetainedCount(),
			len(report.Errors),
		)
	}
}

func (a *App) initESOUI() {
	defer close(a.initDone)
	defer a.initWg.Done()

	var cache *esoui.Cache
	if a.db != nil {
		cache = esoui.NewCacheFromDB(a.db)
	} else {

		var err error
		cache, err = esoui.NewCache()
		if err != nil {
			a.setPersistenceStatus("degraded", privacySafePersistenceError(err))
			wailsRuntime.LogErrorf(a.ctx, "[esoui] cache database unavailable: %s", a.getPersistenceError())
			return
		}
	}
	if a.shutdownCtx.Err() != nil {
		return
	}
	a.esoCache = cache

	client := esoui.NewClient()
	if err := client.Init(); err != nil {
		wailsRuntime.LogErrorf(a.ctx, "[esoui] client.Init failed: %v", err)
		return
	}
	if a.shutdownCtx.Err() != nil {
		return
	}
	a.esoClient = client

	if !cache.IsStale() {
		addons, _, cats := cache.Get()
		if addons != nil {
			a.remoteMu.Lock()
			a.remoteList = addons
			a.remoteCategories = cats
			a.lastRefreshErr = ""
			a.remoteMu.Unlock()
			a.markRemoteReady()
			return
		}
	}

	if a.shutdownCtx.Err() != nil {
		return
	}

	if err := a.refreshRemoteList(); err != nil {
		wailsRuntime.LogErrorf(a.ctx, "[esoui] initial refresh failed: %v", err)
		return
	}
	a.markRemoteReady()
}

func (a *App) domReady(ctx context.Context) {
	a.perfMu.Lock()
	if a.domReadyAt.IsZero() {
		a.domReadyAt = time.Now()
	}
	a.perfMu.Unlock()
	a.logPerformanceSnapshot("dom-ready")
}

func (a *App) markFrontendReady() {
	a.perfMu.Lock()
	if !a.frontendReadyAt.IsZero() {
		a.perfMu.Unlock()
		return
	}
	a.frontendReadyAt = time.Now()
	a.perfMu.Unlock()
	a.logPerformanceSnapshot("frontend-ready")
}

func (a *App) markRemoteReady() {
	a.perfMu.Lock()
	if !a.remoteReadyAt.IsZero() {
		a.perfMu.Unlock()
		return
	}
	a.remoteReadyAt = time.Now()
	a.perfMu.Unlock()
	a.logPerformanceSnapshot("remote-ready")
}

func (a *App) markCachedStateReady() {
	a.scanMu.Lock()
	if a.cachedStateReadyAt.IsZero() {
		a.cachedStateReadyAt = time.Now()
	}
	a.scanMu.Unlock()
}

func (a *App) setPersistenceStatus(status, message string) {
	a.persistenceMu.Lock()
	a.persistenceStatus = status
	a.persistenceError = message
	a.persistenceMu.Unlock()
}

func (a *App) getPersistenceStatus() (string, string) {
	a.persistenceMu.RLock()
	defer a.persistenceMu.RUnlock()
	status := a.persistenceStatus
	if status == "" {
		status = "unknown"
	}
	return status, a.persistenceError
}

func (a *App) getPersistenceError() string {
	_, message := a.getPersistenceStatus()
	return message
}

func privacySafePersistenceError(err error) string {
	if err == nil {
		return ""
	}
	return "settings and cache persistence are unavailable; check user config directory permissions and disk space"
}

func privacySafeInstallCleanupError(message string) string {
	if message == "" {
		return ""
	}
	return "some Scribe-owned temporary install artifacts could not be cleaned; check AddOns directory permissions"
}

func (a *App) logPerformanceSnapshot(label string) {
	if a.ctx == nil {
		return
	}

	snapshot := a.getDiagnosticsSnapshot()

	wailsRuntime.LogInfof(
		a.ctx,
		"[perf] %s startup_ms=%d dom_ready_ms=%d frontend_ready_ms=%d remote_ready_ms=%d heap_alloc_mb=%d sys_mb=%d goroutines=%d remote_addons=%d installed_addons=%d num_gc=%d detail_requests=%d remote_refreshes=%d heap_inuse_mb=%d stack_inuse_mb=%d total_alloc_mb=%d mem_budget_ok=%v startup_budget_ok=%v",
		label,
		snapshot.StartupMS,
		snapshot.DOMReadyMS,
		snapshot.FrontendReadyMS,
		snapshot.RemoteReadyMS,
		snapshot.HeapAllocMB,
		snapshot.SysMB,
		snapshot.Goroutines,
		snapshot.RemoteAddons,
		snapshot.InstalledAddons,
		snapshot.NumGC,
		snapshot.DetailRequests,
		snapshot.RemoteRefreshCount,
		snapshot.HeapInUseMB,
		snapshot.StackInUseMB,
		snapshot.TotalAllocMB,
		snapshot.MemoryBudgetOK,
		snapshot.StartupBudgetOK,
	)

	if !snapshot.StartupBudgetOK && !a.frontendReadyAt.IsZero() {
		wailsRuntime.LogInfof(a.ctx, "[perf] WARNING: startup budget exceeded — frontend_ready_ms=%d (target: <1000ms)", snapshot.FrontendReadyMS)
	}
	if !snapshot.MemoryBudgetOK {
		wailsRuntime.LogInfof(a.ctx, "[perf] WARNING: memory budget exceeded — sys_mb=%d (target: <150MB)", snapshot.SysMB)
	}
}

func (a *App) recordDetailFetch(uid string) {
	a.perfMu.Lock()
	a.detailFetches[uid]++
	a.lastDetailUID = uid
	a.lastDetailAt = time.Now()
	a.perfMu.Unlock()
}

func (a *App) getDiagnosticsSnapshot() DiagnosticsSnapshot {
	a.perfMu.RLock()
	startedAt := a.startedAt
	domReadyAt := a.domReadyAt
	frontendReadyAt := a.frontendReadyAt
	remoteReadyAt := a.remoteReadyAt
	lastDetailUID := a.lastDetailUID
	lastDetailAt := a.lastDetailAt
	remoteRefreshes := a.remoteRefreshes
	lastRefreshAt := a.lastRefreshAt
	lastRefreshMS := a.lastRefreshMS
	detailFetches := make(map[string]int, len(a.detailFetches))
	for uid, count := range a.detailFetches {
		detailFetches[uid] = count
	}
	a.perfMu.RUnlock()

	var mem stdRuntime.MemStats
	stdRuntime.ReadMemStats(&mem)

	installedCount := 0
	if a.scanner != nil {
		installedCount = len(a.scanner.GetAddons())
	}

	a.remoteMu.RLock()
	remoteCount := len(a.remoteList)
	categoryCount := len(a.remoteCategories)
	a.remoteMu.RUnlock()

	detailTop := make([]DiagnosticsCount, 0, len(detailFetches))
	detailRequests := 0
	for uid, count := range detailFetches {
		detailRequests += count
		detailTop = append(detailTop, DiagnosticsCount{Name: uid, Count: count})
	}
	sort.Slice(detailTop, func(i, j int) bool {
		if detailTop[i].Count != detailTop[j].Count {
			return detailTop[i].Count > detailTop[j].Count
		}
		return detailTop[i].Name < detailTop[j].Name
	})
	if len(detailTop) > 8 {
		detailTop = detailTop[:8]
	}

	cacheStale := false
	if a.esoCache != nil {
		cacheStale = a.esoCache.IsStale()
	}
	persistenceStatus, persistenceError := a.getPersistenceStatus()
	a.scanMu.RLock()
	cachedStateReadyAt := a.cachedStateReadyAt
	scanStartedAt := a.scanStartedAt
	scanReadyAt := a.scanReadyAt
	scanInFlight := a.scanInFlight
	lastScanErr := a.lastScanErr
	a.scanMu.RUnlock()
	a.tempCleanupMu.RLock()
	tempCleanupRemoved := a.tempCleanupRemoved
	tempCleanupRetained := a.tempCleanupRetained
	tempCleanupError := a.tempCleanupError
	a.tempCleanupMu.RUnlock()

	now := time.Now()
	startupReadyAt := firstNonZeroTime(frontendReadyAt, domReadyAt, cachedStateReadyAt, now)

	return DiagnosticsSnapshot{
		StartupMS:           elapsedMS(startedAt, startupReadyAt),
		UptimeMS:            elapsedMS(startedAt, now),
		DOMReadyMS:          elapsedMS(startedAt, domReadyAt),
		FrontendReadyMS:     elapsedMS(startedAt, frontendReadyAt),
		RemoteReadyMS:       elapsedMS(startedAt, remoteReadyAt),
		HeapAllocMB:         mem.HeapAlloc / (1024 * 1024),
		SysMB:               mem.Sys / (1024 * 1024),
		Goroutines:          stdRuntime.NumGoroutine(),
		NumGC:               mem.NumGC,
		RemoteAddons:        remoteCount,
		RemoteCategories:    categoryCount,
		InstalledAddons:     installedCount,
		RemoteCacheStale:    cacheStale,
		DetailRequests:      detailRequests,
		DetailUniqueUIDs:    len(detailFetches),
		LastDetailUID:       lastDetailUID,
		LastDetailAt:        formatTime(lastDetailAt),
		DetailTop:           detailTop,
		RemoteRefreshCount:  remoteRefreshes,
		LastRemoteRefreshAt: formatTime(lastRefreshAt),
		LastRemoteRefreshMS: lastRefreshMS,
		HeapInUseMB:         mem.HeapInuse / (1024 * 1024),
		StackInUseMB:        mem.StackInuse / (1024 * 1024),
		TotalAllocMB:        mem.TotalAlloc / (1024 * 1024),
		MemoryBudgetOK:      mem.Sys/(1024*1024) <= 150,
		StartupBudgetOK:     elapsedMS(startedAt, frontendReadyAt) <= 1000 || frontendReadyAt.IsZero(),
		PersistenceStatus:   persistenceStatus,
		PersistenceError:    persistenceError,
		CachedStateReadyMS:  elapsedMS(startedAt, cachedStateReadyAt),
		ScanStartedMS:       elapsedMS(startedAt, scanStartedAt),
		ScanReadyMS:         elapsedMS(startedAt, scanReadyAt),
		ScanInFlight:        scanInFlight,
		LastScanError:       lastScanErr,
		TempCleanupRemoved:  tempCleanupRemoved,
		TempCleanupRetained: tempCleanupRetained,
		TempCleanupError:    tempCleanupError,
	}
}

func elapsedMS(start, end time.Time) int64 {
	if start.IsZero() || end.IsZero() {
		return 0
	}
	return end.Sub(start).Milliseconds()
}

func firstNonZeroTime(values ...time.Time) time.Time {
	for _, value := range values {
		if !value.IsZero() {
			return value
		}
	}
	return time.Time{}
}

func formatTime(t time.Time) string {
	if t.IsZero() {
		return ""
	}
	return t.UTC().Format(time.RFC3339)
}

func (a *App) ensureClient() error {
	if a.esoClient != nil {
		return nil
	}
	client := esoui.NewClient()
	if err := client.Init(); err != nil {
		return fmt.Errorf("ESOUI client init: %w", err)
	}
	a.esoClient = client
	wailsRuntime.LogInfof(a.ctx, "[esoui] client re-initialized successfully")
	return nil
}

func (a *App) refreshRemoteList() error {
	if err := a.ensureClient(); err != nil {
		return err
	}
	addons, err := a.esoClient.FetchAddonList()
	if err != nil {
		return err
	}
	cats, catErr := a.esoClient.FetchCategories()
	if catErr != nil {
		cats = nil
	}
	feeds := a.esoClient.FeedURLs()
	if !a.applyRemoteCatalog(addons, cats, feeds) {
		return context.Canceled
	}
	return nil
}

func (a *App) applyRemoteCatalog(addons []esoui.RemoteAddon, cats []esoui.Category, feeds *esoui.APIFeeds) bool {
	if a.shutdownRequested() {
		return false
	}
	if feeds != nil && a.esoCache != nil {
		_ = a.esoCache.Set(*feeds, addons, cats)
	}
	a.remoteMu.Lock()
	a.remoteList = addons
	a.remoteCategories = cats
	a.lastRefreshErr = ""
	a.remoteMu.Unlock()
	return true
}

func (a *App) shutdownRequested() bool {
	return a.shutdownCtx != nil && a.shutdownCtx.Err() != nil
}

func (a *App) getRemoteList() []esoui.RemoteAddon {
	a.remoteMu.RLock()
	defer a.remoteMu.RUnlock()
	out := make([]esoui.RemoteAddon, len(a.remoteList))
	copy(out, a.remoteList)
	return out
}

func (a *App) configureScannerCache() {
	if a.scanner != nil && a.db != nil {
		a.scanner.SetCacheStore(esoui.NewScannerCacheStore(a.db))
	}
}

func (a *App) beginAddonScan() bool {
	if a.scanner == nil || a.scanner.GetAddonPath() == "" {
		return false
	}

	a.scanMu.Lock()
	defer a.scanMu.Unlock()
	if a.scanInFlight {
		return false
	}
	a.scanInFlight = true
	a.scanStartedAt = time.Now()
	a.scanReadyAt = time.Time{}
	a.lastScanErr = ""
	return true
}

func (a *App) finishAddonScan(count int, err error) string {
	safeErr := ""
	if err != nil {
		safeErr = privacySafePersistenceError(err)
	}

	a.scanMu.Lock()
	a.scanInFlight = false
	a.scanReadyAt = time.Now()
	a.lastScanErr = safeErr
	a.scanMu.Unlock()

	if a.ctx != nil && !a.shutdownRequested() {
		payload := map[string]any{
			"count": count,
		}
		if err != nil {
			payload["error"] = a.lastScanErr
		}
		wailsRuntime.EventsEmit(a.ctx, "installed:scan-complete", payload)
	}
	return safeErr
}

func (a *App) startBackgroundAddonScan(reason string) bool {
	if !a.beginAddonScan() {
		return false
	}

	go func() {
		addons, err := a.scanner.Scan()
		if a.shutdownRequested() {
			return
		}
		safeErr := a.finishAddonScan(len(addons), err)
		if err != nil && a.ctx != nil {
			wailsRuntime.LogErrorf(a.ctx, "[scanner] background scan %s failed: %s", reason, safeErr)
		}
	}()
	return true
}

func (a *App) GetInstalledAddons() ([]*addon.Addon, error) {
	if a.scanner == nil {
		return []*addon.Addon{}, nil
	}
	cached := a.scanner.GetAddons()
	a.startBackgroundAddonScan("get-installed")
	return cached, nil
}

func (a *App) GetAddonPath() string {
	if a.scanner == nil {
		return ""
	}
	return a.scanner.GetAddonPath()
}

func (a *App) SetAddonPath(path string) error {
	if a.scanner == nil {
		a.scanner = scanner.New(path)
	} else {
		a.scanner.SetAddonPath(path)
	}
	a.configureScannerCache()
	if _, err := a.scanner.Scan(); err != nil {
		return err
	}
	if a.settingsMgr == nil {
		return nil
	}

	s, err := a.settingsMgr.GetSettings()
	if err != nil {
		s = settings.AppSettings{MemoryLimitMB: 150, Theme: "scribe"}
	}
	s.AddonPath = path
	return a.settingsMgr.SaveSettings(s)
}

func (a *App) DetectAddonPath() string {
	return scanner.DetectAddonPath()
}

func (a *App) BrowseFolder(title string) (string, error) {
	path, err := wailsRuntime.OpenDirectoryDialog(a.ctx, wailsRuntime.OpenDialogOptions{
		Title: title,
	})
	return path, err
}

func (a *App) OpenPath(path string) error {
	addonPath := ""
	if a.scanner != nil {
		addonPath = a.scanner.GetAddonPath()
	}
	cleaned, err := validateOpenPath(addonPath, path)
	if err != nil {
		return err
	}

	var cmd *exec.Cmd
	switch stdRuntime.GOOS {
	case "windows":
		cmd = exec.Command("explorer", cleaned)
	case "darwin":
		cmd = exec.Command("open", cleaned)
	default:
		cmd = exec.Command("xdg-open", cleaned)
	}
	if err := cmd.Start(); err != nil {
		return err
	}
	go cmd.Wait()
	return nil
}

func validateOpenPath(addonPath, path string) (string, error) {
	cleanRoot := filepath.Clean(strings.TrimSpace(addonPath))
	cleanTarget := filepath.Clean(strings.TrimSpace(path))
	if cleanRoot == "." || cleanTarget == "." {
		return "", fmt.Errorf("addon path is not configured")
	}
	if !filepath.IsAbs(cleanRoot) || !filepath.IsAbs(cleanTarget) {
		return "", fmt.Errorf("path must be absolute")
	}

	rootInfo, err := os.Stat(cleanRoot)
	if err != nil {
		return "", fmt.Errorf("stat addon path: %w", err)
	}
	if !rootInfo.IsDir() {
		return "", fmt.Errorf("addon path is not a directory")
	}
	targetInfo, err := os.Stat(cleanTarget)
	if err != nil {
		return "", fmt.Errorf("stat open path: %w", err)
	}
	if !targetInfo.IsDir() {
		return "", fmt.Errorf("open path is not a directory")
	}

	resolvedRoot, err := filepath.EvalSymlinks(cleanRoot)
	if err != nil {
		return "", fmt.Errorf("resolve addon path: %w", err)
	}
	resolvedTarget, err := filepath.EvalSymlinks(cleanTarget)
	if err != nil {
		return "", fmt.Errorf("resolve open path: %w", err)
	}

	rel, err := filepath.Rel(resolvedRoot, resolvedTarget)
	if err != nil {
		return "", fmt.Errorf("compare open path: %w", err)
	}
	if rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) || filepath.IsAbs(rel) {
		return "", fmt.Errorf("open path must be inside configured AddOns directory")
	}
	return resolvedTarget, nil
}

func (a *App) PerformMemoryCleanup() DiagnosticsSnapshot {
	stdRuntime.GC()
	debug.FreeOSMemory()
	return a.getDiagnosticsSnapshot()
}

func (a *App) GetRemoteAddons() ([]esoui.RemoteAddon, error) {
	<-a.initDone

	list := a.getRemoteList()
	if len(list) == 0 {
		if err := a.refreshRemoteList(); err != nil {
			return nil, fmt.Errorf("unable to reach ESOUI — %w", err)
		}
		list = a.getRemoteList()
	} else if a.esoCache != nil && a.esoCache.IsStale() {
		if !a.beginRemoteRefresh() {
			return list, nil
		}
		a.refreshWg.Add(1)
		go func() {
			defer a.refreshWg.Done()
			defer a.endRemoteRefresh()
			if a.shutdownCtx.Err() != nil {
				return
			}
			if err := a.refreshRemoteList(); err != nil {
				a.remoteMu.Lock()
				a.lastRefreshErr = err.Error()
				a.remoteMu.Unlock()
				wailsRuntime.LogInfof(a.ctx, "[esoui] background refresh failed: %v", err)
			}
		}()
	}
	return list, nil
}

func (a *App) beginRemoteRefresh() bool {
	a.remoteMu.Lock()
	defer a.remoteMu.Unlock()
	if a.refreshInFlight {
		return false
	}
	a.refreshInFlight = true
	a.refreshStarted = time.Now()
	a.lastRefreshErr = ""
	return true
}

func (a *App) endRemoteRefresh() {
	a.remoteMu.Lock()
	a.refreshInFlight = false
	a.remoteMu.Unlock()
}

func (a *App) GetRemoteCatalogStatus() RemoteCatalogStatus {
	<-a.initDone

	a.remoteMu.RLock()
	hasData := len(a.remoteList) > 0
	lastErr := a.lastRefreshErr
	refreshInFlight := a.refreshInFlight
	refreshStarted := a.refreshStarted
	a.remoteMu.RUnlock()

	return RemoteCatalogStatus{
		HasData:          hasData,
		CacheStale:       a.esoCache != nil && a.esoCache.IsStale(),
		LastRefreshError: lastErr,
		RefreshInFlight:  refreshInFlight,
		RefreshStartedAt: formatTime(refreshStarted),
	}
}

func (a *App) RefreshRemoteAddons() ([]esoui.RemoteAddon, error) {
	if !a.beginRemoteRefresh() {
		return a.getRemoteList(), nil
	}
	defer a.endRemoteRefresh()
	if a.esoCache != nil {
		a.esoCache.Invalidate()
	}
	if err := a.refreshRemoteList(); err != nil {
		return nil, fmt.Errorf("refresh remote addons: %w", err)
	}
	return a.getRemoteList(), nil
}

func (a *App) SearchRemoteAddons(query string) ([]esoui.RemoteAddon, error) {
	list := a.getRemoteList()
	return esoui.SearchRemote(list, query), nil
}

func (a *App) GetAddonDetails(uid string) (*esoui.RemoteAddonDetails, error) {
	a.recordDetailFetch(uid)
	<-a.initDone
	if a.esoClient == nil {
		return nil, fmt.Errorf("ESOUI client not ready")
	}
	details, err := a.esoClient.FetchAddonDetails([]string{uid})
	if err != nil {
		return nil, err
	}
	if len(details) == 0 {
		return nil, fmt.Errorf("no details found for UID %s", uid)
	}
	return &details[0], nil
}

func (a *App) GetDiagnostics() DiagnosticsSnapshot {
	return a.getDiagnosticsSnapshot()
}

type AppInfo struct {
	Version        string `json:"version"`
	Commit         string `json:"commit"`
	BuildDate      string `json:"buildDate"`
	GoVersion      string `json:"goVersion"`
	OS             string `json:"os"`
	Arch           string `json:"arch"`
	CustomTitleBar bool   `json:"customTitleBar"`
}

func (a *App) GetAppInfo() AppInfo {
	return AppInfo{
		Version:   a.version,
		Commit:    a.commit,
		BuildDate: a.buildDate,
		GoVersion: stdRuntime.Version(),
		OS:        stdRuntime.GOOS,
		Arch:      stdRuntime.GOARCH,
		// GTK/KDE can still draw native chrome around a frameless Wails window.
		// Prefer the platform titlebar on Linux to avoid duplicate controls.
		CustomTitleBar: stdRuntime.GOOS != "linux",
	}
}

func (a *App) CheckForUpdates() ([]esoui.MatchedAddon, error) {
	<-a.initDone

	if a.scanner == nil {
		return nil, nil
	}
	locals, err := a.scanner.Scan()
	if err != nil {
		return nil, err
	}

	remotes := a.getRemoteList()
	if len(remotes) == 0 {
		return nil, nil
	}

	all := esoui.MatchAddons(locals, remotes)
	all = a.suppressMD5FalsePositives(all)

	var updates []esoui.MatchedAddon
	for _, m := range all {
		if m.UpdateAvailable {
			updates = append(updates, m)
		}
	}
	return updates, nil
}

func (a *App) GetMatchedAddons() ([]esoui.MatchedAddon, error) {
	<-a.initDone

	if a.scanner == nil {
		return nil, nil
	}
	locals, err := a.scanner.Scan()
	if err != nil {
		return nil, err
	}

	remotes := a.getRemoteList()
	if len(remotes) == 0 {
		return nil, nil
	}

	matched := esoui.MatchAddons(locals, remotes)
	return a.suppressMD5FalsePositives(matched), nil
}

func (a *App) suppressMD5FalsePositives(matched []esoui.MatchedAddon) []esoui.MatchedAddon {
	if a.db == nil || a.esoClient == nil {
		return matched
	}

	var updateUIDs []string
	for _, m := range matched {
		if m.Remote != nil {
			updateUIDs = append(updateUIDs, m.Remote.UID)
		}
	}
	if len(updateUIDs) == 0 {
		return matched
	}

	storedMD5s := esoui.GetInstallMD5s(a.db, updateUIDs)
	if len(storedMD5s) == 0 {
		return matched
	}

	var checkUIDs []string
	for _, uid := range updateUIDs {
		if storedMD5s[uid] != "" {
			checkUIDs = append(checkUIDs, uid)
		}
	}
	if len(checkUIDs) == 0 {
		return matched
	}

	details, err := a.esoClient.FetchAddonDetails(checkUIDs)
	if err != nil {
		return matched
	}

	remoteMD5s := make(map[string]string, len(details))
	for _, d := range details {
		remoteMD5s[d.RemoteAddon.UID] = d.UIMD5
	}

	return suppressMD5Matches(matched, storedMD5s, remoteMD5s)
}

func suppressMD5Matches(matched []esoui.MatchedAddon, storedMD5s, remoteMD5s map[string]string) []esoui.MatchedAddon {
	for i := range matched {
		m := &matched[i]
		if m.Remote == nil {
			continue
		}
		uid := m.Remote.UID
		if stored, ok := storedMD5s[uid]; ok && stored != "" {
			if remote, ok2 := remoteMD5s[uid]; ok2 && remote != "" && stored == remote {
				m.UpdateAvailable = false
				m.UpdateState = esoui.UpdateStateUpToDate
				m.UpdateReason = "Installed download MD5 matches ESOUI, so Scribe is suppressing a version-text false positive."
			} else if ok2 && remote != "" && !m.UpdateAvailable && m.UpdateState == esoui.UpdateStateUpToDate {
				m.UpdateAvailable = true
				m.UpdateState = esoui.UpdateStateMD5OnlyChanged
				m.UpdateReason = "ESOUI download MD5 changed while the version text stayed the same."
			}
		}
	}
	return matched
}

// Deprecated: Call InstallAddon to get queued installs with progress updates.
func (a *App) DownloadAndInstall(uid string) error {
	if a.scanner == nil {
		return fmt.Errorf("addon path not configured")
	}
	addonPath := a.scanner.GetAddonPath()
	if addonPath == "" {
		return fmt.Errorf("addon path not configured")
	}

	<-a.initDone
	if a.esoClient == nil {
		return fmt.Errorf("ESOUI client not ready")
	}

	details, err := a.esoClient.FetchAddonDetails([]string{uid})
	if err != nil {
		return fmt.Errorf("fetch details for %s: %w", uid, err)
	}
	if len(details) == 0 {
		return fmt.Errorf("no details found for UID %s", uid)
	}

	downloadURL := details[0].UIDownload
	if downloadURL == "" {
		return fmt.Errorf("no download URL for UID %s", uid)
	}

	return esoui.DownloadAndInstall(downloadURL, addonPath)
}

func (a *App) UninstallAddon(folderName string) error {
	if a.scanner == nil {
		return fmt.Errorf("addon path not configured")
	}
	addonPath := a.scanner.GetAddonPath()
	if addonPath == "" {
		return fmt.Errorf("addon path not configured")
	}
	return esoui.RemoveAddonFolder(addonPath, folderName)
}

func (a *App) InstallAddon(uid string) error {
	if a.scanner == nil {
		return fmt.Errorf("addon path not configured")
	}
	addonPath := a.scanner.GetAddonPath()
	if addonPath == "" {
		return fmt.Errorf("addon path not configured")
	}

	<-a.initDone
	if a.esoClient == nil {
		return fmt.Errorf("ESOUI client not ready")
	}

	details, err := a.esoClient.FetchAddonDetails([]string{uid})
	if err != nil {
		return fmt.Errorf("fetch details for %s: %w", uid, err)
	}
	if len(details) == 0 {
		return fmt.Errorf("no details found for UID %s", uid)
	}

	d := details[0]
	if d.UIDownload == "" {
		return fmt.Errorf("no download URL for UID %s", uid)
	}

	a.downloads.EnqueueWithExpectedDirs(uid, d.UIName, d.UIDownload, d.UIMD5, addonPath, d.UIDirs)
	return nil
}

func (a *App) BatchInstall(uids []string) (int, error) {
	if a.scanner == nil {
		return 0, fmt.Errorf("addon path not configured")
	}
	addonPath := a.scanner.GetAddonPath()
	if addonPath == "" {
		return 0, fmt.Errorf("addon path not configured")
	}

	<-a.initDone
	if a.esoClient == nil {
		return 0, fmt.Errorf("ESOUI client not ready")
	}

	details, err := a.esoClient.FetchAddonDetails(uids)
	if err != nil {
		return 0, fmt.Errorf("fetch details: %w", err)
	}

	queued := 0
	for _, d := range details {
		if d.UIDownload == "" {
			continue
		}
		a.downloads.EnqueueWithExpectedDirs(d.UID, d.UIName, d.UIDownload, d.UIMD5, addonPath, d.UIDirs)
		queued++
	}
	return queued, nil
}

func (a *App) CancelInstall(uid string) {
	a.downloads.Cancel(uid)
}

func (a *App) CancelAllInstalls() {
	a.downloads.CancelAll()
}

func (a *App) GetDownloadQueue() []esoui.TaskProgress {
	return a.downloads.GetAllStatuses()
}

func (a *App) GetCategories() ([]esoui.Category, error) {

	<-a.initDone

	a.remoteMu.RLock()
	cats := a.remoteCategories
	a.remoteMu.RUnlock()
	if len(cats) > 0 {
		return cats, nil
	}

	if a.esoClient == nil {
		return nil, fmt.Errorf("ESOUI client not ready")
	}
	fetched, err := a.esoClient.FetchCategories()
	if err != nil {
		return nil, err
	}
	a.remoteMu.Lock()
	a.remoteCategories = fetched
	a.remoteMu.Unlock()
	return fetched, nil
}

func stripDepVersion(dep string) string {
	if idx := strings.IndexAny(dep, "><!="); idx >= 0 {
		return strings.TrimSpace(dep[:idx])
	}
	return strings.TrimSpace(dep)
}

func depVersionConstraint(dep string) string {
	if idx := strings.IndexAny(dep, "><!="); idx >= 0 {
		return strings.TrimSpace(dep[idx:])
	}
	return ""
}

func (a *App) GetMissingDependencies() ([]esoui.MissingDepInfo, error) {
	if a.scanner == nil {
		return nil, nil
	}
	locals, err := a.scanner.Scan()
	if err != nil {
		return nil, fmt.Errorf("scan addons: %w", err)
	}

	return findMissingDependencies(locals, a.getRemoteList()), nil
}

func findMissingDependencies(locals []*addon.Addon, remotes []esoui.RemoteAddon) []esoui.MissingDepInfo {
	installedNames := make(map[string]struct{}, len(locals))
	for _, local := range locals {
		installedNames[strings.ToLower(local.FolderName)] = struct{}{}
	}

	type depEntry struct {
		requiredBy  []string
		constraints map[string]struct{}
		optional    bool
	}
	missing := make(map[string]*depEntry)

	for _, local := range locals {
		for _, dep := range local.DependsOn {
			folder := strings.ToLower(stripDepVersion(dep))
			if folder == "" {
				continue
			}
			if _, installed := installedNames[folder]; installed {
				continue
			}
			if missing[folder] == nil {
				missing[folder] = &depEntry{constraints: make(map[string]struct{})}
			}
			missing[folder].requiredBy = append(missing[folder].requiredBy, local.FolderName)
			missing[folder].optional = false
			if constraint := depVersionConstraint(dep); constraint != "" {
				missing[folder].constraints[constraint] = struct{}{}
			}
		}

		for _, dep := range local.OptionalDependsOn {
			folder := strings.ToLower(stripDepVersion(dep))
			if folder == "" {
				continue
			}
			if _, installed := installedNames[folder]; installed {
				continue
			}
			if missing[folder] == nil {
				missing[folder] = &depEntry{optional: true, constraints: make(map[string]struct{})}
			}
			missing[folder].requiredBy = append(missing[folder].requiredBy, local.FolderName)
			if constraint := depVersionConstraint(dep); constraint != "" {
				missing[folder].constraints[constraint] = struct{}{}
			}
		}
	}

	if len(missing) == 0 {
		return nil
	}

	dirToRemote := make(map[string]esoui.RemoteAddon, len(remotes))
	for i := range remotes {
		r := remotes[i]
		for _, dir := range r.UIDirs {
			key := strings.ToLower(strings.TrimSpace(dir))
			if key == "" {
				continue
			}
			best, ok := dirToRemote[key]
			if !ok {
				dirToRemote[key] = r
				continue
			}
			if selected, ok := esoui.BestRemoteForDir([]esoui.RemoteAddon{best, r}, key); ok && selected.UID == r.UID {
				dirToRemote[key] = r
			}
		}
	}

	result := make([]esoui.MissingDepInfo, 0, len(missing))
	for folder, entry := range missing {
		info := esoui.MissingDepInfo{
			DepFolderName:      folder,
			RequiredBy:         sortedUniqueStrings(entry.requiredBy),
			VersionConstraints: sortedMapKeys(entry.constraints),
			Optional:           entry.optional,
			PlanState:          "unresolved",
			PlanReason:         "No ESOUI catalog entry matched this dependency folder.",
		}
		if r, ok := dirToRemote[folder]; ok {
			info.RemoteUID = r.UID
			info.RemoteName = r.UIName
			info.CanInstall = true
			info.PlanState = "installable"
			info.PlanReason = "Matched the latest canonical ESOUI addon entry; dependency version constraints are informational and do not pin downloads."
		}
		result = append(result, info)
	}
	sort.Slice(result, func(i, j int) bool {
		if result[i].Optional != result[j].Optional {
			return !result[i].Optional
		}
		return result[i].DepFolderName < result[j].DepFolderName
	})
	return result
}

func sortedUniqueStrings(values []string) []string {
	if len(values) == 0 {
		return nil
	}
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if value != "" {
			seen[value] = struct{}{}
		}
	}
	return sortedMapKeys(seen)
}

func sortedMapKeys(values map[string]struct{}) []string {
	if len(values) == 0 {
		return nil
	}
	out := make([]string, 0, len(values))
	for value := range values {
		out = append(out, value)
	}
	sort.Strings(out)
	return out
}

func (a *App) GetSettings() (settings.AppSettings, error) {
	if a.settingsMgr == nil {
		return settings.AppSettings{MemoryLimitMB: 150}, nil
	}
	return a.settingsMgr.GetSettings()
}

func (a *App) SaveSettings(s settings.AppSettings) error {
	if a.settingsMgr == nil {
		return fmt.Errorf("settings manager not initialised")
	}

	currentPath := ""
	if a.scanner != nil {
		currentPath = a.scanner.GetAddonPath()
	}
	if s.AddonPath != "" && s.AddonPath != currentPath {
		if a.scanner == nil {
			a.scanner = scanner.New(s.AddonPath)
		} else {
			a.scanner.SetAddonPath(s.AddonPath)
		}
		_, _ = a.scanner.Scan()
	}
	return a.settingsMgr.SaveSettings(s)
}

type SearchPreset struct {
	ID             string `json:"id"`
	Name           string `json:"name"`
	SearchQuery    string `json:"searchQuery"`
	CategoryFilter string `json:"categoryFilter"`
	SortBy         string `json:"sortBy"`
	HideInstalled  bool   `json:"hideInstalled"`
	CreatedAt      string `json:"createdAt"`
}

func (a *App) ListSearchPresets() ([]SearchPreset, error) {
	if a.db == nil {
		return []SearchPreset{}, nil
	}
	var rows []esoui.DBSearchPreset
	if err := a.db.Order("name").Find(&rows).Error; err != nil {
		return nil, fmt.Errorf("list search presets: %w", err)
	}
	out := make([]SearchPreset, len(rows))
	for i, r := range rows {
		out[i] = SearchPreset{
			ID:             r.ID,
			Name:           r.Name,
			SearchQuery:    r.SearchQuery,
			CategoryFilter: r.CategoryFilter,
			SortBy:         r.SortBy,
			HideInstalled:  r.HideInstalled,
			CreatedAt:      r.CreatedAt,
		}
	}
	return out, nil
}

func (a *App) SaveSearchPreset(name, searchQuery, categoryFilter, sortBy string, hideInstalled bool) (SearchPreset, error) {
	if a.db == nil {
		return SearchPreset{}, fmt.Errorf("database not initialised")
	}

	var existing esoui.DBSearchPreset
	err := a.db.Where("name = ?", name).First(&existing).Error
	now := time.Now().UTC().Format(time.RFC3339)
	if err != nil {
		row := esoui.DBSearchPreset{
			ID:             uuid.NewString(),
			Name:           name,
			SearchQuery:    searchQuery,
			CategoryFilter: categoryFilter,
			SortBy:         sortBy,
			HideInstalled:  hideInstalled,
			CreatedAt:      now,
		}
		if err2 := a.db.Create(&row).Error; err2 != nil {
			return SearchPreset{}, fmt.Errorf("create search preset: %w", err2)
		}
		return SearchPreset{
			ID:             row.ID,
			Name:           row.Name,
			SearchQuery:    row.SearchQuery,
			CategoryFilter: row.CategoryFilter,
			SortBy:         row.SortBy,
			HideInstalled:  row.HideInstalled,
			CreatedAt:      row.CreatedAt,
		}, nil
	}

	existing.SearchQuery = searchQuery
	existing.CategoryFilter = categoryFilter
	existing.SortBy = sortBy
	existing.HideInstalled = hideInstalled
	if err2 := a.db.Save(&existing).Error; err2 != nil {
		return SearchPreset{}, fmt.Errorf("update search preset: %w", err2)
	}
	return SearchPreset{
		ID:             existing.ID,
		Name:           existing.Name,
		SearchQuery:    existing.SearchQuery,
		CategoryFilter: existing.CategoryFilter,
		SortBy:         existing.SortBy,
		HideInstalled:  existing.HideInstalled,
		CreatedAt:      existing.CreatedAt,
	}, nil
}

func (a *App) DeleteSearchPreset(id string) error {
	if a.db == nil {
		return fmt.Errorf("database not initialised")
	}
	return a.db.Delete(&esoui.DBSearchPreset{}, "id = ?", id).Error
}
