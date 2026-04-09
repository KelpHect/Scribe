package main

import (
	"context"
	"net/http"
	_ "net/http/pprof"
	"os"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

func maybeStartPprof(ctx context.Context) {
	if os.Getenv("SCRIBEEGO_PPROF") != "1" {
		return
	}
	go func() {
		addr := "localhost:6060"
		wailsRuntime.LogInfof(ctx, "[pprof] profiling server on http://%s/debug/pprof/", addr)
		if err := http.ListenAndServe(addr, nil); err != nil {
			wailsRuntime.LogInfof(ctx, "[pprof] server stopped: %v", err)
		}
	}()
}
