package esoui

import (
	"context"
	"crypto/md5" //nolint:gosec // esoui only gives us md5 hashes, so this is integrity checking not crypto
	"encoding/hex"
	"fmt"
	"io"
	"net/http"
	"os"
	"sync"
	"time"
)

type TaskState string

const (
	StateQueued      TaskState = "queued"
	StateDownloading TaskState = "downloading"
	StateExtracting  TaskState = "extracting"
	StateComplete    TaskState = "complete"
	StateFailed      TaskState = "failed"
	StateCancelled   TaskState = "cancelled"
)

type TaskProgress struct {
	UID             string    `json:"uid"`
	Name            string    `json:"name"`
	State           TaskState `json:"state"`
	Percent         float64   `json:"percent"`
	BytesDownloaded int64     `json:"bytesDownloaded"`
	TotalBytes      int64     `json:"totalBytes"`
	Speed           float64   `json:"speed"`
	Error           string    `json:"error,omitempty"`
	FilesExtracted  int       `json:"filesExtracted"`
	TotalFiles      int       `json:"totalFiles"`
	QueuePosition   int       `json:"queuePosition"`
}

type ProgressEmitter func(eventName string, data any)

type downloadTask struct {
	uid       string
	name      string
	url       string
	md5       string
	destDir   string
	cancel    context.CancelFunc
	ctx       context.Context
	startedAt time.Time
}

const (
	defaultConcurrency = 3
	progressInterval   = 100 * time.Millisecond
	queuedTaskTimeout  = 10 * time.Minute
)

type DownloadManager struct {
	concurrency int
	emit        ProgressEmitter
	// runs on the worker goroutine after a successful install
	OnComplete func(uid, md5Hash string)

	mu       sync.Mutex
	queue    []*downloadTask
	active   map[string]*downloadTask
	statuses map[string]*TaskProgress

	sem chan struct{}
	wg  sync.WaitGroup
}

func NewDownloadManager(concurrency int, emit ProgressEmitter) *DownloadManager {
	if concurrency <= 0 {
		concurrency = defaultConcurrency
	}
	return &DownloadManager{
		concurrency: concurrency,
		emit:        emit,
		active:      make(map[string]*downloadTask),
		statuses:    make(map[string]*TaskProgress),
		sem:         make(chan struct{}, concurrency),
	}
}

func (dm *DownloadManager) Enqueue(uid, name, url, md5Hash, destDir string) {
	ctx, cancel := context.WithTimeout(context.Background(), queuedTaskTimeout)
	task := &downloadTask{
		uid:     uid,
		name:    name,
		url:     url,
		md5:     md5Hash,
		destDir: destDir,
		cancel:  cancel,
		ctx:     ctx,
	}

	dm.mu.Lock()

	for _, t := range dm.queue {
		if t.uid == uid {
			dm.mu.Unlock()
			cancel()
			return
		}
	}
	if _, ok := dm.active[uid]; ok {
		dm.mu.Unlock()
		cancel()
		return
	}
	dm.queue = append(dm.queue, task)
	dm.updateQueuePositions()
	dm.emitStatus(uid, &TaskProgress{
		UID:           uid,
		Name:          name,
		State:         StateQueued,
		QueuePosition: len(dm.queue),
	})
	dm.mu.Unlock()

	dm.wg.Add(1)
	go dm.processNext()
}

func (dm *DownloadManager) Cancel(uid string) {
	dm.mu.Lock()
	defer dm.mu.Unlock()

	for i, t := range dm.queue {
		if t.uid == uid {
			t.cancel()
			dm.queue = append(dm.queue[:i], dm.queue[i+1:]...)
			dm.emitStatus(uid, &TaskProgress{
				UID:   uid,
				Name:  t.name,
				State: StateCancelled,
			})
			dm.updateQueuePositions()
			dm.wg.Done()
			return
		}
	}

	if t, ok := dm.active[uid]; ok {
		t.cancel()

	}
}

func (dm *DownloadManager) CancelAll() {
	dm.mu.Lock()
	for _, t := range dm.queue {
		t.cancel()
		dm.emitStatus(t.uid, &TaskProgress{
			UID:   t.uid,
			Name:  t.name,
			State: StateCancelled,
		})
		dm.wg.Done()
	}
	dm.queue = nil

	for _, t := range dm.active {
		t.cancel()
	}
	dm.mu.Unlock()
}

func (dm *DownloadManager) GetAllStatuses() []TaskProgress {
	dm.mu.Lock()
	defer dm.mu.Unlock()
	out := make([]TaskProgress, 0, len(dm.statuses))
	for _, s := range dm.statuses {
		out = append(out, *s)
	}
	return out
}

func (dm *DownloadManager) Shutdown() {
	dm.CancelAll()
	dm.wg.Wait()
}

func (dm *DownloadManager) processNext() {

	dm.sem <- struct{}{}
	defer func() { <-dm.sem }()
	defer dm.wg.Done()

	dm.mu.Lock()
	if len(dm.queue) == 0 {
		dm.mu.Unlock()
		return
	}
	task := dm.queue[0]
	dm.queue = dm.queue[1:]
	dm.active[task.uid] = task
	dm.updateQueuePositions()
	dm.mu.Unlock()

	task.startedAt = time.Now()
	err := dm.runTask(task)

	dm.mu.Lock()
	delete(dm.active, task.uid)
	dm.mu.Unlock()

	if err != nil {
		if task.ctx.Err() != nil {
			dm.emitStatusLocked(task.uid, &TaskProgress{
				UID:   task.uid,
				Name:  task.name,
				State: StateCancelled,
			})
		} else {
			dm.emitStatusLocked(task.uid, &TaskProgress{
				UID:   task.uid,
				Name:  task.name,
				State: StateFailed,
				Error: err.Error(),
			})
		}
	} else {
		dm.emitStatusLocked(task.uid, &TaskProgress{
			UID:     task.uid,
			Name:    task.name,
			State:   StateComplete,
			Percent: 100,
		})

		if dm.OnComplete != nil && task.md5 != "" {
			dm.OnComplete(task.uid, task.md5)
		}
	}
}

func (dm *DownloadManager) runTask(task *downloadTask) error {

	dm.emitStatusLocked(task.uid, &TaskProgress{
		UID:   task.uid,
		Name:  task.name,
		State: StateDownloading,
	})

	tmpPath, err := dm.downloadFile(task)
	if err != nil {
		return err
	}
	defer os.Remove(tmpPath)

	if task.md5 != "" {
		if err := verifyMD5(tmpPath, task.md5); err != nil {
			return err
		}
	}

	dm.emitStatusLocked(task.uid, &TaskProgress{
		UID:     task.uid,
		Name:    task.name,
		State:   StateExtracting,
		Percent: 0,
	})

	return ExtractWithProgress(task.ctx, tmpPath, task.destDir, func(extracted, total int) {
		pct := float64(0)
		if total > 0 {
			pct = float64(extracted) / float64(total) * 100
		}
		dm.emitStatusLocked(task.uid, &TaskProgress{
			UID:            task.uid,
			Name:           task.name,
			State:          StateExtracting,
			Percent:        pct,
			FilesExtracted: extracted,
			TotalFiles:     total,
		})
	})
}

func (dm *DownloadManager) downloadFile(task *downloadTask) (string, error) {
	tmp, err := os.CreateTemp("", "scribeeso_*.zip")
	if err != nil {
		return "", fmt.Errorf("create temp file: %w", err)
	}
	tmpPath := tmp.Name()

	req, err := http.NewRequestWithContext(task.ctx, http.MethodGet, task.url, nil)
	if err != nil {
		tmp.Close()
		os.Remove(tmpPath)
		return "", fmt.Errorf("create request: %w", err)
	}

	client := &http.Client{}
	resp, err := client.Do(req)
	if err != nil {
		tmp.Close()
		os.Remove(tmpPath)
		return "", fmt.Errorf("download %s: %w", task.url, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		tmp.Close()
		os.Remove(tmpPath)
		return "", fmt.Errorf("download %s returned %d", task.url, resp.StatusCode)
	}

	totalBytes := resp.ContentLength
	if totalBytes < 0 {
		totalBytes = 0
	}

	pw := NewProgressWriter(tmp, totalBytes, func(written, total int64) {
		pct := float64(0)
		if total > 0 {
			pct = float64(written) / float64(total) * 100
		}
		elapsed := time.Since(task.startedAt).Seconds()
		speed := float64(0)
		if elapsed > 0 {
			speed = float64(written) / elapsed
		}
		dm.emitStatusLocked(task.uid, &TaskProgress{
			UID:             task.uid,
			Name:            task.name,
			State:           StateDownloading,
			Percent:         pct,
			BytesDownloaded: written,
			TotalBytes:      total,
			Speed:           speed,
		})
	}, progressInterval)

	if _, err := io.Copy(pw, resp.Body); err != nil {
		tmp.Close()
		os.Remove(tmpPath)
		return "", fmt.Errorf("write download: %w", err)
	}
	pw.Finish()

	if err := tmp.Close(); err != nil {
		os.Remove(tmpPath)
		return "", fmt.Errorf("close temp file: %w", err)
	}

	return tmpPath, nil
}

func verifyMD5(filePath, expectedHex string) error {
	f, err := os.Open(filePath)
	if err != nil {
		return fmt.Errorf("open file for MD5 check: %w", err)
	}
	defer f.Close()

	h := md5.New() //nolint:gosec
	if _, err := io.Copy(h, f); err != nil {
		return fmt.Errorf("compute MD5: %w", err)
	}
	actual := hex.EncodeToString(h.Sum(nil))
	if actual != expectedHex {
		return fmt.Errorf("MD5 mismatch: expected %s, got %s", expectedHex, actual)
	}
	return nil
}

func (dm *DownloadManager) emitStatus(uid string, p *TaskProgress) {
	dm.statuses[uid] = p
	if dm.emit != nil {
		dm.emit("download:progress", *p)
	}
}

func (dm *DownloadManager) emitStatusLocked(uid string, p *TaskProgress) {
	dm.mu.Lock()
	dm.statuses[uid] = p
	dm.mu.Unlock()
	if dm.emit != nil {
		dm.emit("download:progress", *p)
	}
}

func (dm *DownloadManager) updateQueuePositions() {
	for i, t := range dm.queue {
		if s, ok := dm.statuses[t.uid]; ok {
			s.QueuePosition = i + 1
		}
	}
}
