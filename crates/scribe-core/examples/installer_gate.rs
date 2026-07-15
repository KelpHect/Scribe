use std::alloc::System;
use std::io::Write;
use std::time::{Duration, Instant};

use scribe_core::{CancellationToken, Installer};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats, StatsAlloc};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const FILES: usize = 1_000;

fn main() {
    let temp = tempfile::tempdir().expect("temporary installer benchmark directory");
    let archive_path = temp.path().join("addon.zip");
    let file = std::fs::File::create(&archive_path).expect("create archive fixture");
    let mut archive = ZipWriter::new(file);
    archive
        .start_file("BenchAddon/BenchAddon.txt", SimpleFileOptions::default())
        .expect("start manifest");
    archive
        .write_all(b"## Title: Bench Addon\n## Version: 1.0\n")
        .expect("write manifest");
    for index in 1..FILES {
        archive
            .start_file(
                format!("BenchAddon/data/file-{index:04}.lua"),
                SimpleFileOptions::default(),
            )
            .expect("start fixture entry");
        archive
            .write_all(b"local value = 1234567890\n")
            .expect("write fixture entry");
    }
    archive.finish().expect("finish archive fixture");

    let addons_path = temp.path().join("AddOns");
    std::fs::create_dir(&addons_path).expect("create AddOns destination");
    let expected = vec!["BenchAddon".to_owned()];
    let (plan, plan_time, plan_alloc) = measure(|| {
        Installer::plan_archive(&archive_path, &addons_path, &expected).expect("archive preflight")
    });
    assert_eq!(plan.len(), 1);
    let (_, install_time, install_alloc) = measure(|| {
        Installer::install_archive(
            &archive_path,
            &addons_path,
            &expected,
            &CancellationToken::default(),
            |_, _| {},
        )
        .expect("safe archive install")
    });
    assert!(addons_path.join("BenchAddon/BenchAddon.txt").is_file());

    println!("Scribe installer release gate ({FILES} archive files)");
    print_metric("safe archive preflight", plan_time, plan_alloc);
    print_metric("staging + atomic commit", install_time, install_alloc);
}

fn measure<T>(operation: impl FnOnce() -> T) -> (T, Duration, Stats) {
    let region = Region::new(GLOBAL);
    let started = Instant::now();
    let value = operation();
    (value, started.elapsed(), region.change())
}

fn print_metric(label: &str, elapsed: Duration, stats: Stats) {
    println!(
        "{label}: {:.3} ms; {} allocations; {} bytes allocated",
        elapsed.as_secs_f64() * 1_000.0,
        stats.allocations,
        stats.bytes_allocated,
    );
}
