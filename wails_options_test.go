package main

import "testing"

func TestUseCustomTitleBarForGOOS(t *testing.T) {
	tests := []struct {
		goos string
		want bool
	}{
		{goos: "windows", want: true},
		{goos: "darwin", want: true},
		{goos: "linux", want: false},
		{goos: "freebsd", want: true},
	}

	for _, tt := range tests {
		t.Run(tt.goos, func(t *testing.T) {
			if got := useCustomTitleBarForGOOS(tt.goos); got != tt.want {
				t.Fatalf("useCustomTitleBarForGOOS(%q) = %v, want %v", tt.goos, got, tt.want)
			}
		})
	}
}
