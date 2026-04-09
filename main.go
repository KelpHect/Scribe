package main

import (
	"embed"
	"fmt"
	"os"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/windows"
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

	err := wails.Run(&options.App{
		Title:     "Scribe",
		Width:     1120,
		Height:    768,
		MinWidth:  800,
		MinHeight: 600,
		Frameless: true,
		AssetServer: &assetserver.Options{
			Assets: assets,
		},
		BackgroundColour: &options.RGBA{R: 9, G: 9, B: 11, A: 255},
		OnStartup:        app.startup,
		OnDomReady:       app.domReady,
		OnShutdown:       app.shutdown,
		Windows: &windows.Options{
			WebviewIsTransparent: false,
			WindowIsTranslucent:  false,
		},
		Bind: []interface{}{
			app,
		},
	})

	if err != nil {
		_, _ = fmt.Fprintf(os.Stderr, "Scribe fatal: %v\n", err)
		os.Exit(1)
	}
}
