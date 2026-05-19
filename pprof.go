package main

import (
	"context"
	"log"
	"net/http"
	_ "net/http/pprof"
	"os"
)

const (
	pprofEnv       = "SCRIBE_PPROF"
	legacyPprofEnv = "SCRIBEEGO_PPROF"
)

func maybeStartPprof(ctx context.Context) {
	_ = ctx
	if !pprofEnabled() {
		return
	}
	go func() {
		addr := "localhost:6060"
		log.Printf("[pprof] profiling server on http://%s/debug/pprof/", addr)
		if err := http.ListenAndServe(addr, nil); err != nil {
			log.Printf("[pprof] server stopped: %v", err)
		}
	}()
}

func pprofEnabled() bool {
	return os.Getenv(pprofEnv) == "1" || os.Getenv(legacyPprofEnv) == "1"
}
