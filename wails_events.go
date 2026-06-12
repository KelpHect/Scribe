package main

import (
	"Scribe/internal/esoui"

	"github.com/wailsapp/wails/v3/pkg/application"
)

func init() {
	application.RegisterEvent[esoui.TaskProgress]("download:progress")
	application.RegisterEvent[[]esoui.TaskProgress]("download:progress-batch")
	application.RegisterEvent[InstalledScanComplete]("installed:scan-complete")
	application.RegisterEvent[RemoteCatalogStatus]("remote-catalog:status")
	application.RegisterEvent[application.Void]("perf:frontend-ready")
	application.RegisterEvent[string]("perf:capture")
}
