package main

import (
	"testing"

	"Scribe/internal/esoui"
)

func TestShouldBatchDownloadProgressOnlyForCounterUpdates(t *testing.T) {
	tests := []struct {
		name     string
		progress esoui.TaskProgress
		want     bool
	}{
		{
			name:     "queued state transition",
			progress: esoui.TaskProgress{State: esoui.StateQueued},
		},
		{
			name:     "download start transition",
			progress: esoui.TaskProgress{State: esoui.StateDownloading},
		},
		{
			name:     "download byte update",
			progress: esoui.TaskProgress{State: esoui.StateDownloading, BytesDownloaded: 1024},
			want:     true,
		},
		{
			name:     "extract start transition",
			progress: esoui.TaskProgress{State: esoui.StateExtracting},
		},
		{
			name:     "extract file update",
			progress: esoui.TaskProgress{State: esoui.StateExtracting, FilesExtracted: 1, TotalFiles: 5},
			want:     true,
		},
		{
			name:     "complete transition",
			progress: esoui.TaskProgress{State: esoui.StateComplete, Percent: 100},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := shouldBatchDownloadProgress(tt.progress); got != tt.want {
				t.Fatalf("shouldBatchDownloadProgress() = %v, want %v", got, tt.want)
			}
		})
	}
}
