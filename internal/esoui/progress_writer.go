package esoui

import (
	"io"
	"sync"
	"time"
)

type ProgressFn func(written, total int64)

type ProgressWriter struct {
	dst      io.Writer
	total    int64
	fn       ProgressFn
	interval time.Duration

	mu       sync.Mutex
	written  int64
	lastEmit time.Time
}

func NewProgressWriter(dst io.Writer, total int64, fn ProgressFn, interval time.Duration) *ProgressWriter {
	if interval <= 0 {
		interval = 100 * time.Millisecond
	}
	return &ProgressWriter{
		dst:      dst,
		total:    total,
		fn:       fn,
		interval: interval,
	}
}

func (pw *ProgressWriter) Write(p []byte) (int, error) {
	n, err := pw.dst.Write(p)
	if n > 0 {
		pw.mu.Lock()
		pw.written += int64(n)
		now := time.Now()
		shouldEmit := now.Sub(pw.lastEmit) >= pw.interval
		written := pw.written
		pw.mu.Unlock()

		if shouldEmit && pw.fn != nil {
			pw.mu.Lock()
			pw.lastEmit = now
			pw.mu.Unlock()
			pw.fn(written, pw.total)
		}
	}
	return n, err
}

func (pw *ProgressWriter) Finish() {
	pw.mu.Lock()
	written := pw.written
	pw.mu.Unlock()
	if pw.fn != nil {
		pw.fn(written, pw.total)
	}
}
