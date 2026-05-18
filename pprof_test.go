package main

import "testing"

func TestPprofEnabledEnvNames(t *testing.T) {
	t.Setenv(pprofEnv, "")
	t.Setenv(legacyPprofEnv, "")
	if pprofEnabled() {
		t.Fatal("pprofEnabled() = true without env")
	}

	t.Setenv(pprofEnv, "1")
	if !pprofEnabled() {
		t.Fatal("pprofEnabled() = false with SCRIBE_PPROF=1")
	}

	t.Setenv(pprofEnv, "")
	t.Setenv(legacyPprofEnv, "1")
	if !pprofEnabled() {
		t.Fatal("pprofEnabled() = false with legacy SCRIBEEGO_PPROF=1")
	}
}
