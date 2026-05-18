package scanner

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func BenchmarkScanLargeAddOnsDirectory(b *testing.B) {
	root := b.TempDir()
	for i := 0; i < 1000; i++ {
		folder := fmt.Sprintf("Addon%04d", i)
		dir := filepath.Join(root, folder)
		if err := os.MkdirAll(dir, 0o755); err != nil {
			b.Fatal(err)
		}
		manifest := fmt.Sprintf("## Title: Addon %04d\n## Version: 1.%d\n## Author: Bench\n", i, i)
		if i%3 == 0 {
			manifest += "## DependsOn: LibShared LibOptional>=2\n"
		}
		if err := os.WriteFile(filepath.Join(dir, folder+".txt"), []byte(manifest), 0o644); err != nil {
			b.Fatal(err)
		}
	}

	s := New(root)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := s.Scan(); err != nil {
			b.Fatal(err)
		}
	}
}
