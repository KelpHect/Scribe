use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rkyv::rancor::Error;
use scribe_core::{Catalog, CatalogArchive, CatalogIndex, Category, GameVersion, RemoteAddon};

const ITEMS: usize = 7_000;
const SAMPLES: usize = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = fixture();
    let postcard = postcard::to_allocvec(&catalog)?;
    let archived = rkyv::to_bytes::<Error>(&catalog)?;

    for _ in 0..10 {
        black_box(postcard::from_bytes::<Catalog>(&postcard)?);
        black_box(rkyv::from_bytes::<Catalog, Error>(&archived)?);
    }

    let postcard_decode = sample(|| {
        black_box(postcard::from_bytes::<Catalog>(black_box(&postcard)).unwrap());
    });
    let rkyv_decode = sample(|| {
        // from_bytes validates with bytecheck before producing an owned Catalog.
        black_box(rkyv::from_bytes::<Catalog, Error>(black_box(&archived)).unwrap());
    });
    let postcard_encode = sample(|| {
        black_box(postcard::to_allocvec(black_box(&catalog)).unwrap());
    });
    let rkyv_encode = sample(|| {
        black_box(rkyv::to_bytes::<Error>(black_box(&catalog)).unwrap());
    });
    let postcard_ready = sample(|| {
        let catalog = Arc::new(postcard::from_bytes::<Catalog>(black_box(&postcard)).unwrap());
        let index = CatalogIndex::new(catalog);
        let visible = index.search("author 17");
        black_box(
            visible
                .iter()
                .take(40)
                .filter_map(|position| index.addon(*position))
                .collect::<Vec<_>>(),
        );
    });
    let archive_ready = sample(|| {
        let archive = CatalogArchive::from_bytes(black_box(&archived)).unwrap();
        archive.with_catalog(|catalog| {
            let index = ArchivedIndex::new(catalog);
            let visible = index.search(catalog, "author 17");
            black_box(
                visible
                    .iter()
                    .take(40)
                    .map(|index| archive.addon_owned(*index).unwrap().unwrap())
                    .collect::<Vec<_>>(),
            );
        });
    });

    let saved = postcard_decode.saturating_sub(rkyv_decode);
    let percent = if postcard_decode.is_zero() {
        0.0
    } else {
        saved.as_secs_f64() / postcard_decode.as_secs_f64() * 100.0
    };
    let codec_gate = saved >= Duration::from_millis(2) && percent >= 20.0;

    println!("Scribe reconstructible catalog codec gate ({ITEMS} addons, median of {SAMPLES})");
    println!("postcard bytes: {}", postcard.len());
    println!("rkyv bytes:     {}", archived.len());
    println!("postcard decode: {:.3} ms", millis(postcard_decode));
    println!("rkyv decode:     {:.3} ms", millis(rkyv_decode));
    println!("postcard encode: {:.3} ms", millis(postcard_encode));
    println!("rkyv encode:     {:.3} ms", millis(rkyv_encode));
    println!(
        "postcard ready + query + 40 rows: {:.3} ms",
        millis(postcard_ready)
    );
    println!(
        "archive copy + validate + index + query + 40 rows: {:.3} ms",
        millis(archive_ready)
    );
    println!(
        "archive-backed ready-path saved: {:.3} ms",
        millis(postcard_ready.saturating_sub(archive_ready))
    );
    println!("decode saved:    {:.3} ms ({percent:.1}%)", millis(saved));
    println!("codec threshold (>=20% and >=2ms): {codec_gate}");
    println!(
        "The ready-path measurement, memory, and correctness now decide archive-backed adoption; owned-decode thresholds do not veto further work."
    );
    Ok(())
}

struct ArchivedIndex {
    search_rows: Vec<usize>,
}

impl ArchivedIndex {
    fn new(catalog: &rkyv::Archived<Catalog>) -> Self {
        let mut search_rows: Vec<_> = (0..catalog.addons.len()).collect();
        search_rows.sort_unstable_by(|left, right| {
            catalog.addons[*left]
                .uid
                .as_str()
                .cmp(catalog.addons[*right].uid.as_str())
        });
        Self { search_rows }
    }

    fn search(&self, catalog: &rkyv::Archived<Catalog>, query: &str) -> Vec<usize> {
        let query = query.trim().to_ascii_lowercase();
        self.search_rows
            .iter()
            .copied()
            .filter(|index| {
                let addon = &catalog.addons[*index];
                contains_ascii_case_insensitive(addon.ui_name.as_str(), &query)
                    || contains_ascii_case_insensitive(addon.ui_author_name.as_str(), &query)
                    || contains_ascii_case_insensitive(addon.uid.as_str(), &query)
                    || addon.ui_dirs.iter().any(|directory| {
                        contains_ascii_case_insensitive(directory.as_str(), &query)
                    })
            })
            .collect()
    }
}

fn contains_ascii_case_insensitive(haystack: &str, lowercase_needle: &str) -> bool {
    let needle = lowercase_needle.as_bytes();
    needle.is_empty()
        || haystack.as_bytes().windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(needle)
                .all(|(left, right)| left.to_ascii_lowercase() == *right)
        })
}

fn sample(mut operation: impl FnMut()) -> Duration {
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        operation();
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    samples[SAMPLES / 2]
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn fixture() -> Catalog {
    let addons = (0..ITEMS)
        .map(|index| RemoteAddon {
            uid: index.to_string().into(),
            category_id: (index % 40).to_string().into(),
            ui_name: format!("Addon {index:04}").into(),
            ui_author_name: format!("Author {}", index % 200).into(),
            ui_date: format!("2026-{:02}-{:02}", index % 12 + 1, index % 28 + 1).into(),
            ui_version: format!("{}.{}.{}", index % 20, index % 15, index % 9).into(),
            ui_dirs: vec![format!("Addon{index:04}").into()],
            ui_file_info_url: format!("https://www.esoui.com/downloads/info{index}.html").into(),
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
        .collect();
    let categories = (0..40)
        .map(|index| Category {
            id: index.to_string().into(),
            name: format!("Category {index}").into(),
            count: (ITEMS / 40) as i32,
            ..Category::default()
        })
        .collect();
    Catalog { addons, categories }
}
