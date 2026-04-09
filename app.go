package main

import (
	"context"
	"fmt"
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

	frontendReadyOff func()
	perfCaptureOff   func()

	db *gorm.DB

	esoClient        *esoui.Client
	esoCache         *esoui.Cache
	remoteMu         sync.RWMutex
	remoteList       []esoui.RemoteAddon
	remoteCategories []esoui.Category

	downloads *esoui.DownloadManager

	settingsMgr *settings.Manager

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

	if detectedPath != "" {
		_, _ = a.scanner.Scan()
	}

	if db, err := esoui.OpenAppDB(); err == nil {
		a.db = db

		a.settingsMgr = settings.NewManager(db)

		if detectedPath == "" {
			if s, err2 := a.settingsMgr.GetSettings(); err2 == nil && s.AddonPath != "" {
				a.scanner = scanner.New(s.AddonPath)
				_, _ = a.scanner.Scan()
			}
		}
	}

	a.shutdownCtx, a.shutdownCancel = context.WithCancel(context.Background())
	maybeStartPprof(a.shutdownCtx)
	a.initWg.Add(1)
	go a.initESOUI()
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
			wailsRuntime.LogErrorf(a.ctx, "[esoui] cache creation failed: %v", err)
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

func (a *App) recordRemoteRefresh(started time.Time) {
	now := time.Now()
	a.perfMu.Lock()
	a.remoteRefreshes++
	a.lastRefreshAt = now
	a.lastRefreshMS = now.Sub(started).Milliseconds()
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

	return DiagnosticsSnapshot{
		StartupMS:           elapsedMS(startedAt, time.Now()),
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
	}
}

func elapsedMS(start, end time.Time) int64 {
	if start.IsZero() || end.IsZero() {
		return 0
	}
	return end.Sub(start).Milliseconds()
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
	if feeds != nil && a.esoCache != nil {
		_ = a.esoCache.Set(*feeds, addons, cats)
	}
	a.remoteMu.Lock()
	a.remoteList = addons
	a.remoteCategories = cats
	a.remoteMu.Unlock()
	return nil
}

func (a *App) getRemoteList() []esoui.RemoteAddon {
	a.remoteMu.RLock()
	defer a.remoteMu.RUnlock()
	out := make([]esoui.RemoteAddon, len(a.remoteList))
	copy(out, a.remoteList)
	return out
}

func (a *App) GetInstalledAddons() ([]*addon.Addon, error) {
	if a.scanner == nil {
		return []*addon.Addon{}, nil
	}
	return a.scanner.Scan()
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
		return nil
	}
	a.scanner.SetAddonPath(path)
	_, err := a.scanner.Scan()
	return err
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
	cleaned := filepath.Clean(path)
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
		a.refreshWg.Add(1)
		go func() {
			defer a.refreshWg.Done()
			if a.shutdownCtx.Err() != nil {
				return
			}
			_ = a.refreshRemoteList()
		}()
	}
	return list, nil
}

func (a *App) RefreshRemoteAddons() ([]esoui.RemoteAddon, error) {
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
	Version   string `json:"version"`
	Commit    string `json:"commit"`
	BuildDate string `json:"buildDate"`
	GoVersion string `json:"goVersion"`
	OS        string `json:"os"`
	Arch      string `json:"arch"`
}

func (a *App) GetAppInfo() AppInfo {
	return AppInfo{
		Version:   a.version,
		Commit:    a.commit,
		BuildDate: a.buildDate,
		GoVersion: stdRuntime.Version(),
		OS:        stdRuntime.GOOS,
		Arch:      stdRuntime.GOARCH,
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
		if m.UpdateAvailable && m.Remote != nil {
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

	for i := range matched {
		m := &matched[i]
		if !m.UpdateAvailable || m.Remote == nil {
			continue
		}
		uid := m.Remote.UID
		if stored, ok := storedMD5s[uid]; ok && stored != "" {
			if remote, ok2 := remoteMD5s[uid]; ok2 && remote != "" && stored == remote {
				m.UpdateAvailable = false
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

	a.downloads.Enqueue(uid, d.UIName, d.UIDownload, d.UIMD5, addonPath)
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
		a.downloads.Enqueue(d.UID, d.UIName, d.UIDownload, d.UIMD5, addonPath)
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

func (a *App) GetMissingDependencies() ([]esoui.MissingDepInfo, error) {
	if a.scanner == nil {
		return nil, nil
	}
	locals, err := a.scanner.Scan()
	if err != nil {
		return nil, fmt.Errorf("scan addons: %w", err)
	}

	installedNames := make(map[string]struct{}, len(locals))
	for _, a := range locals {
		installedNames[strings.ToLower(a.FolderName)] = struct{}{}
	}

	type depEntry struct {
		requiredBy []string
		optional   bool
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
				missing[folder] = &depEntry{}
			}
			missing[folder].requiredBy = append(missing[folder].requiredBy, local.FolderName)
			missing[folder].optional = false
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
				missing[folder] = &depEntry{optional: true}
			}
			missing[folder].requiredBy = append(missing[folder].requiredBy, local.FolderName)
		}
	}

	if len(missing) == 0 {
		return nil, nil
	}

	remotes := a.getRemoteList()
	dirToRemote := make(map[string]*esoui.RemoteAddon, len(remotes))
	for i := range remotes {
		r := &remotes[i]
		for _, dir := range r.UIDirs {
			dirToRemote[strings.ToLower(dir)] = r
		}
	}

	result := make([]esoui.MissingDepInfo, 0, len(missing))
	for folder, entry := range missing {
		info := esoui.MissingDepInfo{
			DepFolderName: folder,
			RequiredBy:    entry.requiredBy,
			Optional:      entry.optional,
		}
		if r, ok := dirToRemote[folder]; ok {
			info.RemoteUID = r.UID
			info.RemoteName = r.UIName
			info.CanInstall = true
		}
		result = append(result, info)
	}
	return result, nil
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
