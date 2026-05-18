package main

import (
	"fmt"
	"testing"

	"Scribe/internal/addon"
	"Scribe/internal/esoui"
)

func BenchmarkMissingDependencyResolution(b *testing.B) {
	locals := make([]*addon.Addon, 2000)
	for i := range locals {
		locals[i] = &addon.Addon{
			FolderName: fmt.Sprintf("Addon%04d", i),
			Title:      fmt.Sprintf("Addon %04d", i),
			DependsOn: []string{
				fmt.Sprintf("LibRequired%02d>=1.0", i%60),
				"LibShared",
			},
			OptionalDependsOn: []string{fmt.Sprintf("LibOptional%02d", i%40)},
		}
	}

	remotes := make([]esoui.RemoteAddon, 80)
	for i := range remotes {
		remotes[i] = esoui.RemoteAddon{
			UID:       fmt.Sprintf("lib-%02d", i),
			UIName:    fmt.Sprintf("Library %02d", i),
			UIVersion: "1.0",
			UIDirs:    []string{fmt.Sprintf("LibRequired%02d", i)},
		}
	}

	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = findMissingDependencies(locals, remotes)
	}
}
