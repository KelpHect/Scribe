package esoui

import (
	"bytes"
	"testing"
	"time"
)

func TestProgressWriterThrottlesByteProgressAndEmitsFinal(t *testing.T) {
	var got []int64
	writer := NewProgressWriter(&bytes.Buffer{}, 10, func(written, _ int64) {
		got = append(got, written)
	}, time.Hour)

	if _, err := writer.Write([]byte("12345")); err != nil {
		t.Fatalf("first write: %v", err)
	}
	if _, err := writer.Write([]byte("67890")); err != nil {
		t.Fatalf("second write: %v", err)
	}
	writer.Finish()

	if len(got) != 2 {
		t.Fatalf("progress events = %v, want initial and final only", got)
	}
	if got[0] != 5 || got[1] != 10 {
		t.Fatalf("progress events = %v, want [5 10]", got)
	}
}

func TestProgressIntervalForActiveCount(t *testing.T) {
	if got := progressIntervalForActiveCount(1); got != singleTaskProgressTick {
		t.Fatalf("single task interval = %s, want %s", got, singleTaskProgressTick)
	}
	if got := progressIntervalForActiveCount(2); got != concurrentProgressTick {
		t.Fatalf("concurrent interval = %s, want %s", got, concurrentProgressTick)
	}
}
