use std::collections::HashSet;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use scribe_core::{Catalog, CatalogIndex, CatalogSort, Category, GameVersion, RemoteAddon};

fn main() {
    let categories = vec![
        Category {
            id: "maps".into(),
            name: "Map, Coords, Compasses".into(),
            count: 3_500,
            ..Category::default()
        },
        Category {
            id: "libraries".into(),
            name: "Libraries".into(),
            count: 3_500,
            ..Category::default()
        },
    ];
    let addons = (0..7_000)
        .map(|index| RemoteAddon {
            uid: format!("addon-{index}").into(),
            category_id: if index % 2 == 0 { "maps" } else { "libraries" }.into(),
            ui_name: format!("Addon {:04}", (index * 919) % 7_000).into(),
            ui_author_name: format!("Author {}", index % 100).into(),
            ui_date: format!("2026-{:02}-{:02}", index % 12 + 1, index % 28 + 1).into(),
            ui_download_total: (7_000 - index) as i64 * 1_000,
            ui_favorite_total: (index % 2_000) as i64,
            compatabilities: vec![GameVersion {
                version: "101049".into(),
                name: "Season Zero".into(),
            }],
            ..RemoteAddon::default()
        })
        .collect();
    let catalog = CatalogIndex::new(Arc::new(Catalog { addons, categories }));
    let hidden = HashSet::new();
    let iterations = 1_000;

    let started = Instant::now();
    for _ in 0..iterations {
        black_box(catalog.filter_sort(
            "map",
            None,
            false,
            None,
            &hidden,
            CatalogSort::Downloads,
            false,
        ));
    }
    let indexed_query = started.elapsed().as_secs_f64() * 1_000.0 / iterations as f64;

    let started = Instant::now();
    for _ in 0..iterations {
        black_box(catalog.filter_sort("", None, false, None, &hidden, CatalogSort::Title, false));
    }
    let full_title_sort = started.elapsed().as_secs_f64() * 1_000.0 / iterations as f64;

    println!("catalog entries: {}", catalog.len());
    println!("indexed query average: {indexed_query:.3} ms");
    println!("full title sort average: {full_title_sort:.3} ms");
}
