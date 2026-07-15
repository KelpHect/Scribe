use std::alloc::System;
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use redb::Builder;
use scribe_core::{
    Addon, Catalog, CatalogIndex, Category, InstallRecord, InstalledIndex, MatchedAddon, Matcher,
    RemoteAddon, SaveOutcome, ScannerCacheRecord, Storage,
};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats, StatsAlloc};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn main() {
    let temp = tempfile::tempdir().expect("temporary benchmark directory");
    let path = temp.path().join("scribe.redb");
    let catalog = fixture(7_000);
    let storage = Storage::open(&path).expect("open benchmark database");

    let (initial, initial_time, initial_alloc) = measure(|| storage.save_catalog(&catalog, 1));
    assert_eq!(initial.expect("initial save"), SaveOutcome::Written);
    let (unchanged, unchanged_time, unchanged_alloc) =
        measure(|| storage.save_catalog(&catalog, 2));
    assert_eq!(unchanged.expect("unchanged save"), SaveOutcome::Unchanged);

    let mut changed = catalog.clone();
    changed.addons[3_500].ui_version = "changed".into();
    let (changed_outcome, changed_time, changed_alloc) =
        measure(|| storage.save_catalog(&changed, 3));
    assert_eq!(changed_outcome.expect("changed save"), SaveOutcome::Written);
    drop(storage);

    let (cold_loaded, cold_time, cold_alloc) = measure(|| {
        let storage = Storage::open(&path).expect("cold open");
        storage
            .load_catalog(4)
            .expect("cold load")
            .expect("catalog")
    });
    assert_eq!(cold_loaded.catalog.len(), 7_000);

    let mut raw_open_times = Vec::with_capacity(30);
    let raw_open_region = Region::new(GLOBAL);
    for _ in 0..30 {
        let started = Instant::now();
        let mut builder = Builder::new();
        builder.set_cache_size(16 * 1024 * 1024);
        let reopened = builder.open(&path).expect("raw redb reopen");
        raw_open_times.push(started.elapsed());
        drop(reopened);
    }
    raw_open_times.sort_unstable();
    let raw_open_alloc = raw_open_region.change();
    let raw_open_median = raw_open_times[raw_open_times.len() / 2];

    let mut reopen_times = Vec::with_capacity(30);
    let reopen_region = Region::new(GLOBAL);
    for _ in 0..30 {
        let started = Instant::now();
        let reopened = Storage::open(&path).expect("reopen benchmark database");
        black_box(reopened.path());
        reopen_times.push(started.elapsed());
        drop(reopened);
    }
    reopen_times.sort_unstable();
    let reopen_alloc = reopen_region.change();
    let reopen_median = reopen_times[reopen_times.len() / 2];

    let storage = Storage::open(&path).expect("warm open");
    let mut warm_times = Vec::with_capacity(30);
    let warm_region = Region::new(GLOBAL);
    for _ in 0..30 {
        let started = Instant::now();
        black_box(
            storage
                .load_catalog(4)
                .expect("warm load")
                .expect("catalog"),
        );
        warm_times.push(started.elapsed());
    }
    warm_times.sort_unstable();
    let warm_alloc = warm_region.change();
    let warm_median = warm_times[warm_times.len() / 2];

    let changed = Arc::new(changed);
    let (index, index_time, index_alloc) =
        measure(|| CatalogIndex::new(black_box(changed.clone())));
    let lookup_keys: Vec<_> = (0..1_000)
        .map(|index_value| (index_value % 7_000).to_string())
        .collect();
    let (_, lookup_time, lookup_alloc) = measure(|| {
        for key in &lookup_keys {
            black_box(index.by_uid(key));
        }
    });
    let (_, filter_time, filter_alloc) = measure(|| black_box(index.search("author 42")));

    let installed: Vec<_> = (0..1_000)
        .map(|index| Addon {
            folder_name: format!("Addon{index}"),
            title: format!("Installed Addon {index}"),
            author: format!("Author {}", index % 50),
            ..Addon::default()
        })
        .collect();
    let matched: Vec<_> = (0..1_000)
        .map(|index| MatchedAddon {
            update_available: index % 3 == 0,
            ..MatchedAddon::default()
        })
        .collect();
    let (_, analysis_time, analysis_alloc) =
        measure(|| black_box(Matcher::analyze_index(&installed, &index)));
    let installed_index = InstalledIndex::new(&installed, &matched);
    let (_, installed_filter_time, installed_filter_alloc) =
        measure(|| black_box(installed_index.search("author 42", true)));

    let scanner_records: Vec<_> = (0..1_000)
        .map(|index| {
            (
                format!("path\0Addon{index}"),
                ScannerCacheRecord {
                    fingerprint: format!("fingerprint-{index}"),
                    addon: installed[index].clone(),
                    ..ScannerCacheRecord::default()
                },
            )
        })
        .collect();
    let scanner_keys: Vec<_> = scanner_records.iter().map(|(key, _)| key.clone()).collect();
    let (_, scanner_time, scanner_alloc) = measure(|| {
        storage
            .put_scanner_records(&scanner_records)
            .expect("scanner cache batch write");
        black_box(
            storage
                .scanner_records(&scanner_keys)
                .expect("scanner cache batch read"),
        );
    });

    let (_, install_time, install_alloc) = measure(|| {
        for index_value in 0..1_000 {
            storage
                .put_install_record(&InstallRecord {
                    uid: index_value.to_string(),
                    md5: format!("md5-{index_value}"),
                })
                .expect("install record write");
        }
        for index_value in 0..1_000 {
            black_box(
                storage
                    .install_record(&index_value.to_string())
                    .expect("install record read"),
            );
        }
    });

    println!(
        "Scribe raw redb + {} release gate (7,000 addons)",
        if cfg!(feature = "rkyv-catalog") {
            "validated rkyv archive"
        } else {
            "owned postcard"
        }
    );
    print_metric("initial save", initial_time, initial_alloc, 1);
    print_metric("unchanged save", unchanged_time, unchanged_alloc, 1);
    print_metric("changed-one save", changed_time, changed_alloc, 1);
    print_metric("cold open + load", cold_time, cold_alloc, 1);
    print_metric(
        "raw redb reopen median",
        raw_open_median,
        raw_open_alloc,
        30,
    );
    print_metric("existing DB reopen median", reopen_median, reopen_alloc, 30);
    print_metric("warm load median", warm_median, warm_alloc, 30);
    print_metric("catalog index build", index_time, index_alloc, 1);
    print_metric("1,000 in-memory UID lookups", lookup_time, lookup_alloc, 1);
    print_metric("indexed filter", filter_time, filter_alloc, 1);
    print_metric(
        "1,000 installed match + dependency analysis",
        analysis_time,
        analysis_alloc,
        1,
    );
    print_metric(
        "installed update filter",
        installed_filter_time,
        installed_filter_alloc,
        1,
    );
    print_metric(
        "1,000 scanner cache batch write + read",
        scanner_time,
        scanner_alloc,
        1,
    );
    print_metric(
        "1,000 redb install writes + reads",
        install_time,
        install_alloc,
        1,
    );
    println!(
        "database size: {} bytes",
        std::fs::metadata(path).expect("database metadata").len()
    );
    println!(
        "filter target (<16.7 ms): {}",
        if filter_time < Duration::from_micros(16_700) {
            "PASS"
        } else {
            "FAIL"
        }
    );
}

fn fixture(count: usize) -> Catalog {
    Catalog {
        addons: (0..count)
            .map(|index| RemoteAddon {
                uid: index.to_string().into(),
                category_id: (index % 30).to_string().into(),
                ui_name: format!("Addon {index}").into(),
                ui_author_name: format!("Author {}", index % 200).into(),
                ui_date: "2026-07-14".into(),
                ui_version: format!("{}.{}.{}", index % 10, index % 20, index % 50).into(),
                ui_dirs: vec![format!("Addon{index}").into()],
                ui_file_info_url: format!("https://example.invalid/{index}").into(),
                ui_download_total: index as i64 * 17,
                ui_img_thumbs: vec![format!("https://example.invalid/{index}.png").into()],
                ..RemoteAddon::default()
            })
            .collect(),
        categories: (0..30)
            .map(|index| Category {
                id: index.to_string().into(),
                name: format!("Category {index}").into(),
                ..Category::default()
            })
            .collect(),
    }
}

fn measure<T>(operation: impl FnOnce() -> T) -> (T, Duration, Stats) {
    let region = Region::new(GLOBAL);
    let started = Instant::now();
    let value = operation();
    let elapsed = started.elapsed();
    (value, elapsed, region.change())
}

fn print_metric(label: &str, elapsed: Duration, stats: Stats, samples: usize) {
    println!(
        "{label}: {:.3} ms; {} allocations; {} bytes allocated{}",
        elapsed.as_secs_f64() * 1_000.0,
        stats.allocations,
        stats.bytes_allocated,
        if samples > 1 {
            format!(" across {samples} samples")
        } else {
            String::new()
        }
    );
}
