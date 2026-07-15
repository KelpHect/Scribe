use std::alloc::System;
use std::hint::black_box;
use std::time::{Duration, Instant};

use rkyv::rancor::Error as RkyvError;
use scribe_core::{Catalog, Category, GameVersion, RemoteAddon};
use serde::{Deserialize, Serialize};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats, StatsAlloc};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const ITEMS: usize = 7_000;
const SAMPLES: usize = 200;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = fixture();
    let postcard = postcard::to_allocvec(&catalog)?;
    let rkyv = rkyv::to_bytes::<RkyvError>(&catalog)?;

    for _ in 0..20 {
        black_box(postcard::from_bytes::<Catalog>(&postcard)?);
        black_box(rkyv::from_bytes::<Catalog, RkyvError>(&rkyv)?);
        black_box(rkyv::access::<rkyv::Archived<Catalog>, RkyvError>(&rkyv)?);
    }

    println!("Scribe catalog codec lab ({ITEMS} addons, median of {SAMPLES})");
    report(
        "postcard",
        postcard.len(),
        || {
            black_box(postcard::from_bytes::<Catalog>(black_box(&postcard)).unwrap());
        },
        || {
            black_box(postcard::to_allocvec(black_box(&catalog)).unwrap());
        },
    );
    let compact: CompactCatalog =
        postcard::from_bytes(&postcard).expect("compact catalog must decode from postcard");
    report(
        "postcard-compact-string",
        postcard.len(),
        || {
            black_box(postcard::from_bytes::<CompactCatalog>(black_box(&postcard)).unwrap());
        },
        || {
            black_box(postcard::to_allocvec(black_box(&compact)).unwrap());
        },
    );
    let smol: SmolCatalog =
        postcard::from_bytes(&postcard).expect("smol catalog must decode from postcard");
    report(
        "postcard-smol-str",
        postcard.len(),
        || {
            black_box(postcard::from_bytes::<SmolCatalog>(black_box(&postcard)).unwrap());
        },
        || {
            black_box(postcard::to_allocvec(black_box(&smol)).unwrap());
        },
    );
    report_clone("compact-string clone 1,000 addons", || {
        black_box(
            compact
                .addons
                .iter()
                .take(1_000)
                .cloned()
                .collect::<Vec<_>>(),
        );
    });
    report_clone("smol-str clone 1,000 addons", || {
        black_box(smol.addons.iter().take(1_000).cloned().collect::<Vec<_>>());
    });
    report(
        "rkyv-validated-view",
        rkyv.len(),
        || {
            black_box(
                rkyv::access::<rkyv::Archived<Catalog>, RkyvError>(black_box(&rkyv)).unwrap(),
            );
        },
        || {
            black_box(rkyv::to_bytes::<RkyvError>(black_box(&catalog)).unwrap());
        },
    );
    report(
        "rkyv-copy-validated-view",
        rkyv.len(),
        || {
            let mut copied = rkyv::util::AlignedVec::<16>::with_capacity(rkyv.len());
            copied.extend_from_slice(black_box(&rkyv));
            black_box(
                rkyv::access::<rkyv::Archived<Catalog>, RkyvError>(black_box(&copied)).unwrap(),
            );
        },
        || {
            black_box(rkyv::to_bytes::<RkyvError>(black_box(&catalog)).unwrap());
        },
    );
    report(
        "rkyv-validated-owned",
        rkyv.len(),
        || {
            black_box(rkyv::from_bytes::<Catalog, RkyvError>(black_box(&rkyv)).unwrap());
        },
        || {
            black_box(rkyv::to_bytes::<RkyvError>(black_box(&catalog)).unwrap());
        },
    );
    Ok(())
}

fn report(name: &str, bytes: usize, mut decode: impl FnMut(), mut encode: impl FnMut()) {
    let (decode, decode_alloc) = sample(&mut decode);
    let (encode, encode_alloc) = sample(&mut encode);
    println!(
        "{name}: {bytes} bytes; decode {:.3} ms ({} alloc, {} bytes); encode {:.3} ms ({} alloc, {} bytes)",
        millis(decode),
        decode_alloc.allocations / SAMPLES,
        decode_alloc.bytes_allocated / SAMPLES,
        millis(encode),
        encode_alloc.allocations / SAMPLES,
        encode_alloc.bytes_allocated / SAMPLES,
    );
}

fn sample(operation: &mut impl FnMut()) -> (Duration, Stats) {
    let mut samples = Vec::with_capacity(SAMPLES);
    let region = Region::new(GLOBAL);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        operation();
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    (samples[SAMPLES / 2], region.change())
}

fn report_clone(name: &str, mut operation: impl FnMut()) {
    let (elapsed, allocations) = sample(&mut operation);
    println!(
        "{name}: {:.3} ms ({} alloc, {} bytes)",
        millis(elapsed),
        allocations.allocations / SAMPLES,
        allocations.bytes_allocated / SAMPLES,
    );
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

type CompactString = compact_str::CompactString;

#[derive(Clone, Serialize, Deserialize)]
struct CompactCatalog {
    addons: Vec<CompactRemoteAddon>,
    categories: Vec<CompactCategory>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CompactGameVersion {
    version: CompactString,
    name: CompactString,
}

#[derive(Clone, Serialize, Deserialize)]
struct CompactRemoteAddon {
    uid: CompactString,
    category_id: CompactString,
    ui_name: CompactString,
    ui_author_name: CompactString,
    ui_date: CompactString,
    ui_version: CompactString,
    ui_dirs: Vec<CompactString>,
    ui_file_info_url: CompactString,
    ui_download_total: i64,
    ui_download_monthly: i64,
    ui_favorite_total: i64,
    ui_img_thumbs: Vec<CompactString>,
    ui_imgs: Vec<CompactString>,
    compatabilities: Vec<CompactGameVersion>,
    siblings: Vec<CompactString>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CompactCategory {
    id: CompactString,
    name: CompactString,
    icon_url: CompactString,
    parent_id: CompactString,
    parent_ids: Vec<CompactString>,
    count: i32,
}

type SmolString = smol_str::SmolStr;

#[derive(Clone, Serialize, Deserialize)]
struct SmolCatalog {
    addons: Vec<SmolRemoteAddon>,
    categories: Vec<SmolCategory>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SmolGameVersion {
    version: SmolString,
    name: SmolString,
}

#[derive(Clone, Serialize, Deserialize)]
struct SmolRemoteAddon {
    uid: SmolString,
    category_id: SmolString,
    ui_name: SmolString,
    ui_author_name: SmolString,
    ui_date: SmolString,
    ui_version: SmolString,
    ui_dirs: Vec<SmolString>,
    ui_file_info_url: SmolString,
    ui_download_total: i64,
    ui_download_monthly: i64,
    ui_favorite_total: i64,
    ui_img_thumbs: Vec<SmolString>,
    ui_imgs: Vec<SmolString>,
    compatabilities: Vec<SmolGameVersion>,
    siblings: Vec<SmolString>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SmolCategory {
    id: SmolString,
    name: SmolString,
    icon_url: SmolString,
    parent_id: SmolString,
    parent_ids: Vec<SmolString>,
    count: i32,
}

fn fixture() -> Catalog {
    Catalog {
        addons: (0..ITEMS)
            .map(|index| RemoteAddon {
                uid: index.to_string().into(),
                category_id: (index % 40).to_string().into(),
                ui_name: format!("Addon {index:04}").into(),
                ui_author_name: format!("Author {}", index % 200).into(),
                ui_date: format!("2026-{:02}-{:02}", index % 12 + 1, index % 28 + 1).into(),
                ui_version: format!("{}.{}.{}", index % 20, index % 15, index % 9).into(),
                ui_dirs: vec![format!("Addon{index:04}").into()],
                ui_file_info_url: format!("https://www.esoui.com/downloads/info{index}.html")
                    .into(),
                ui_download_total: (index * 100) as i64,
                ui_download_monthly: (index * 3) as i64,
                ui_favorite_total: (index / 2) as i64,
                ui_img_thumbs: vec![format!("https://cdn.mmoui.com/thumb/{index}.jpg").into()],
                ui_imgs: vec![format!("https://cdn.mmoui.com/image/{index}.jpg").into()],
                compatabilities: vec![GameVersion {
                    version: "101047".into(),
                    name: "ESO".into(),
                }],
                siblings: Vec::new(),
            })
            .collect(),
        categories: (0..40)
            .map(|index| Category {
                id: index.to_string().into(),
                name: format!("Category {index}").into(),
                count: (ITEMS / 40) as i32,
                ..Category::default()
            })
            .collect(),
    }
}
