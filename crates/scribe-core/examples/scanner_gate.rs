use std::alloc::System;
use std::sync::Arc;
use std::time::{Duration, Instant};

use scribe_core::{Scanner, Storage};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats, StatsAlloc};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const ADDONS: usize = 1_000;

fn main() {
    let temp = tempfile::tempdir().expect("temporary scanner benchmark directory");
    let addons_path = temp.path().join("AddOns");
    std::fs::create_dir(&addons_path).expect("create AddOns fixture");
    for index in 0..ADDONS {
        let directory = addons_path.join(format!("Addon{index}"));
        std::fs::create_dir(&directory).expect("create addon fixture");
        std::fs::write(
            directory.join(format!("Addon{index}.txt")),
            format!(
                "## Title: Addon {index}\n## Version: {}.{}\n## Author: Author {}\n",
                index % 20,
                index % 7,
                index % 50
            ),
        )
        .expect("write addon fixture");
    }

    let storage = Arc::new(Storage::open(temp.path().join("scanner.redb")).expect("open storage"));
    let scanner = Scanner::new(&addons_path).with_storage(storage);
    let (initial, initial_time, initial_alloc) = measure(|| scanner.scan().expect("initial scan"));
    let (cached, cached_time, cached_alloc) = measure(|| scanner.scan().expect("cached scan"));
    assert_eq!(initial.len(), ADDONS);
    assert_eq!(cached.len(), ADDONS);

    println!("Scribe scanner release gate ({ADDONS} installed addon folders)");
    print_metric(
        "initial parse + one cache commit",
        initial_time,
        initial_alloc,
    );
    print_metric(
        "cached fingerprint + one cache read",
        cached_time,
        cached_alloc,
    );
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
