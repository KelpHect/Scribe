package esoui

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestDownloadManagerCancelQueuedTaskDoesNotPanicOrHang(t *testing.T) {
	server := newBlockingDownloadServer()
	defer server.Close()

	dm := NewDownloadManager(1, nil)
	dm.Enqueue("active", "Active", server.URL, "", t.TempDir())
	waitForTaskState(t, dm, "active", StateDownloading)

	dm.Enqueue("queued", "Queued", server.URL, "", t.TempDir())
	waitForTaskState(t, dm, "queued", StateQueued)

	dm.Cancel("queued")
	waitForTaskState(t, dm, "queued", StateCancelled)
	shutdownWithin(t, dm)
}

func TestDownloadManagerCancelAllQueuedTasksDoesNotPanicOrHang(t *testing.T) {
	server := newBlockingDownloadServer()
	defer server.Close()

	dm := NewDownloadManager(1, nil)
	dm.Enqueue("active", "Active", server.URL, "", t.TempDir())
	waitForTaskState(t, dm, "active", StateDownloading)

	dm.Enqueue("queued-1", "Queued 1", server.URL, "", t.TempDir())
	dm.Enqueue("queued-2", "Queued 2", server.URL, "", t.TempDir())
	waitForTaskState(t, dm, "queued-1", StateQueued)
	waitForTaskState(t, dm, "queued-2", StateQueued)

	dm.CancelAll()
	waitForTaskState(t, dm, "queued-1", StateCancelled)
	waitForTaskState(t, dm, "queued-2", StateCancelled)
	shutdownWithin(t, dm)
}

func TestDownloadManagerShutdownWithQueuedTasksDoesNotPanicOrHang(t *testing.T) {
	server := newBlockingDownloadServer()
	defer server.Close()

	dm := NewDownloadManager(1, nil)
	dm.Enqueue("active", "Active", server.URL, "", t.TempDir())
	waitForTaskState(t, dm, "active", StateDownloading)

	dm.Enqueue("queued-1", "Queued 1", server.URL, "", t.TempDir())
	dm.Enqueue("queued-2", "Queued 2", server.URL, "", t.TempDir())
	waitForTaskState(t, dm, "queued-1", StateQueued)
	waitForTaskState(t, dm, "queued-2", StateQueued)

	shutdownWithin(t, dm)
}

func TestDownloadManagerCancelDuringExtractionReportsCancelled(t *testing.T) {
	zipPath := createTestZip(t, map[string]string{
		"Addon/01-first.txt":  "first",
		"Addon/02-second.txt": "second",
	})
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.ServeFile(w, r, zipPath)
	}))
	defer server.Close()

	dest := t.TempDir()
	var dm *DownloadManager
	dm = NewDownloadManager(1, func(_ string, data any) {
		status, ok := data.(TaskProgress)
		if !ok || status.UID != "extract-cancel" {
			return
		}
		if status.State == StateExtracting && status.FilesExtracted == 1 {
			dm.Cancel(status.UID)
		}
	})

	dm.Enqueue("extract-cancel", "Extract Cancel", server.URL, "", dest)
	waitForTaskState(t, dm, "extract-cancel", StateCancelled)
	shutdownWithin(t, dm)

	assertFileContent(t, filepath.Join(dest, "Addon", "01-first.txt"), "first")
	if _, err := os.Stat(filepath.Join(dest, "Addon", "02-second.txt")); !os.IsNotExist(err) {
		t.Fatalf("second file was extracted after task cancellation or stat failed unexpectedly: %v", err)
	}
}

func newBlockingDownloadServer() *httptest.Server {
	return httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		if flusher, ok := w.(http.Flusher); ok {
			flusher.Flush()
		}
		<-r.Context().Done()
	}))
}

func waitForTaskState(t *testing.T, dm *DownloadManager, uid string, state TaskState) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		for _, status := range dm.GetAllStatuses() {
			if status.UID == uid && status.State == state {
				return
			}
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatalf("timed out waiting for task %s to reach state %s; statuses: %+v", uid, state, dm.GetAllStatuses())
}

func shutdownWithin(t *testing.T, dm *DownloadManager) {
	t.Helper()
	done := make(chan struct{})
	go func() {
		dm.Shutdown()
		close(done)
	}()

	select {
	case <-done:
	case <-time.After(2 * time.Second):
		t.Fatalf("timed out waiting for download manager shutdown; statuses: %+v", dm.GetAllStatuses())
	}
}
