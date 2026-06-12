package main

import (
	"embed"
	"fmt"
	"os"
	"runtime"

	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
)

//go:embed all:frontend/dist
var assets embed.FS

var (
	version = "dev"
	commit  = "none"
	date    = "unknown"
)

func main() {
	app := NewApp()
	app.version = version
	app.commit = commit
	app.buildDate = date
	customTitleBar := useCustomTitleBarForGOOS(runtime.GOOS)

	wailsApp := application.New(application.Options{
		Name:        "Scribe",
		Description: "ESO addon manager",
		Assets: application.AssetOptions{
			Handler: application.AssetFileServerFS(assets),
		},
		Services: []application.Service{
			application.NewService(app),
		},
		Mac: application.MacOptions{
			ApplicationShouldTerminateAfterLastWindowClosed: true,
		},
	})
	app.setWailsApp(wailsApp)

	mainWindow := wailsApp.Window.NewWithOptions(application.WebviewWindowOptions{
		Name:             "main",
		Title:            "Scribe",
		Width:            1120,
		Height:           768,
		MinWidth:         800,
		MinHeight:        600,
		Frameless:        customTitleBar,
		BackgroundColour: application.NewRGBA(9, 9, 11, 255),
		URL:              "/",
		Windows: application.WindowsWindow{
			DisableFramelessWindowDecorations: false,
		},
	})
	mainWindow.OnWindowEvent(events.Common.WindowRuntimeReady, func(*application.WindowEvent) {
		app.runtimeReady()
	})

	err := wailsApp.Run()

	if err != nil {
		_, _ = fmt.Fprintf(os.Stderr, "Scribe fatal: %v\n", err)
		os.Exit(1)
	}
}
