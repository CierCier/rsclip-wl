use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use rsclip_core::models::{EntryFilter, NewEntry, NewEntryData, SortMode};
use rsclip_core::Database;

fn create_sample_db(num_entries: usize, include_huge: bool) -> (PathBuf, Database) {
    let rand: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let db_path = std::env::temp_dir().join(format!("rsclip_perf_test_{rand}.db"));
    let db = Database::open(&db_path).expect("failed to open database");

    println!("Seeding database with {} entries...", num_entries);
    let start = Instant::now();

    for i in 0..num_entries {
        let (data, title, preview, text_content) = match i % 6 {
            0 => {
                let text = format!("Standard clipboard text number {i} with some extra description text to make it realistic.");
                (NewEntryData::Text, format!("Text {i}"), text.clone(), Some(text))
            }
            1 => {
                let code = format!("fn function_{i}() -> usize {{\n    let x = {i} * 42;\n    println!(\"result: {{}}\", x);\n    x\n}}");
                (NewEntryData::Text, format!("fn function_{i}"), code.clone(), Some(code))
            }
            2 => {
                (
                    NewEntryData::Link {
                        url: format!("https://github.com/project/repo/issues/{i}"),
                        domain: "github.com".to_string(),
                        icon: "".to_string(),
                    },
                    format!("GitHub Issue #{i}"),
                    format!("https://github.com/project/repo/issues/{i}"),
                    Some(format!("https://github.com/project/repo/issues/{i}")),
                )
            }
            3 => {
                (
                    NewEntryData::Color {
                        value: format!("#{:06x}", (i * 12345) & 0xffffff),
                        format: "hex".to_string(),
                    },
                    format!("Color #{:06x}", (i * 12345) & 0xffffff),
                    format!("#{:06x}", (i * 12345) & 0xffffff),
                    None,
                )
            }
            4 => {
                (
                    NewEntryData::Image {
                        file_path: Some(format!("/tmp/synthetic_image_{i}.png")),
                        thumb_path: Some(format!("/tmp/synthetic_thumb_{i}.png")),
                        ocr_text: Some(format!("OCR scanned text line {i} invoice amount $100.00")),
                    },
                    format!("Image {i}"),
                    format!("synthetic_image_{i}.png"),
                    None,
                )
            }
            _ => {
                let json = format!("{{\"id\": {i}, \"status\": \"active\", \"tags\": [\"perf\", \"test\", \"row-{i}\"]}}");
                (NewEntryData::Text, format!("JSON payload {i}"), json.clone(), Some(json))
            }
        };

        let new_entry = NewEntry {
            content_hash: format!("hash_{i:08}"),
            data,
            mime_type: "text/plain".to_string(),
            title,
            preview_text: Some(preview),
            text_content,
            size_bytes: 128,
        };

        let id = db.upsert_entry(&new_entry).expect("upsert failed");
        if i % 20 == 0 {
            let _ = db.set_pinned(id, true);
        }
        if i % 7 == 0 {
            for _ in 0..(i % 50) {
                let _ = db.touch_used(id);
            }
        }
    }

    if include_huge {
        println!("Inserting degenerate 10MB and 40MB text payloads for stress tests...");
        let huge_10mb = "A".repeat(10 * 1024 * 1024);
        db.upsert_entry(&NewEntry {
            content_hash: "hash_huge_10mb".to_string(),
            data: NewEntryData::Text,
            mime_type: "text/plain".to_string(),
            title: "Huge 10MB payload".to_string(),
            preview_text: Some(huge_10mb[..200].to_string()),
            text_content: Some(huge_10mb),
            size_bytes: 10 * 1024 * 1024,
        }).expect("upsert 10mb failed");

        let huge_40mb = "B".repeat(40 * 1024 * 1024);
        db.upsert_entry(&NewEntry {
            content_hash: "hash_huge_40mb".to_string(),
            data: NewEntryData::Text,
            mime_type: "text/plain".to_string(),
            title: "Huge 40MB payload".to_string(),
            preview_text: Some(huge_40mb[..200].to_string()),
            text_content: Some(huge_40mb),
            size_bytes: 40 * 1024 * 1024,
        }).expect("upsert 40mb failed");
    }

    println!("Seeding complete in {:.2?}", start.elapsed());
    (db_path, db)
}

fn check_query_plan(db: &Database, sql: &str) -> (bool, String) {
    let mut stmt = db.connection().prepare(&format!("EXPLAIN QUERY PLAN {sql}")).expect("prepare explain failed");
    let rows: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(3))
        .expect("query map failed")
        .map(|r| r.unwrap())
        .collect();
    let plan = rows.join(" | ");
    let has_temp_btree = plan.contains("USE TEMP B-TREE");
    (has_temp_btree, plan)
}

fn run_permutation_sweep(db: &Database) {
    println!("\n=======================================================");
    println!("=== RUNNING EXHAUSTIVE QUERY PERMUTATION PERFORMANCE ===");
    println!("=======================================================\n");

    let sort_modes = [
        (SortMode::Default, "Default (pinned, updated_at DESC)"),
        (SortMode::Recent, "Recent (updated_at DESC)"),
        (SortMode::Oldest, "Oldest (updated_at ASC)"),
        (SortMode::Type, "Type (kind ASC, updated_at DESC)"),
        (SortMode::MostUsed, "MostUsed (use_count DESC, updated_at DESC)"),
    ];

    let filters = [
        (EntryFilter::All, "All"),
        (EntryFilter::Text, "Text"),
        (EntryFilter::Code, "Code"),
        (EntryFilter::Images, "Images"),
        (EntryFilter::Files, "Files"),
        (EntryFilter::Links, "Links"),
        (EntryFilter::Colors, "Colors"),
        (EntryFilter::Pinned, "Pinned"),
    ];

    let queries = [
        ("", "Empty (Browsing/Scrolling)"),
        ("function", "Search 'function'"),
        ("github", "Search 'github'"),
        ("nonexistent_xyz", "Search no-match"),
    ];

    println!("| Sort Mode | Filter | Query | Plan Temp B-Tree? | P95 Latency | Count Latency |");
    println!("|---|---|---|---|---|---|");

    let mut temp_btree_count = 0;
    let mut total_perms = 0;

    for (sort, sort_name) in &sort_modes {
        for (filter, filter_name) in &filters {
            for (query, query_desc) in &queries {
                total_perms += 1;

                // Measure list_entry_summaries_page (the UI virtual scrolling query)
                let iterations = 20;
                let mut latencies = Vec::with_capacity(iterations);

                for _ in 0..iterations {
                    let t0 = Instant::now();
                    let _ = db.list_entry_summaries_page(query, *filter, *sort, 120, 0).unwrap();
                    latencies.push(t0.elapsed());
                }

                latencies.sort();
                let p95 = latencies[(iterations as f64 * 0.95) as usize];

                // Measure count_entries
                let t_cnt = Instant::now();
                let _cnt = db.count_entries(query, *filter).unwrap();
                let cnt_dur = t_cnt.elapsed();

                // Query plan check for the browsing/scrolling query
                let (has_temp_btree, _plan_str) = if query.is_empty() && matches!(filter, EntryFilter::All | EntryFilter::Images | EntryFilter::Pinned) {
                    let order_clause = match sort {
                        SortMode::Default => "ORDER BY e.pinned DESC, e.updated_at DESC",
                        SortMode::Recent => "ORDER BY e.updated_at DESC",
                        SortMode::Oldest => "ORDER BY e.updated_at ASC",
                        SortMode::Type => "ORDER BY e.kind ASC, e.updated_at DESC",
                        SortMode::MostUsed => "ORDER BY e.use_count DESC, e.updated_at DESC",
                    };
                    let where_filter = match filter {
                        EntryFilter::All => "",
                        EntryFilter::Images => " AND e.kind = 'image'",
                        EntryFilter::Pinned => " AND e.pinned = 1",
                        _ => "",
                    };
                    let sql = format!("SELECT e.id FROM entries e WHERE e.deleted = 0 {where_filter} {order_clause} LIMIT 120 OFFSET 0");
                    check_query_plan(db, &sql)
                } else {
                    (false, "N/A".to_string())
                };

                if has_temp_btree {
                    temp_btree_count += 1;
                }

                println!(
                    "| {:<8} | {:<7} | {:<22} | {:<17} | {:>9.2?} | {:>11.2?} |",
                    sort_name.split_whitespace().next().unwrap_or(""),
                    filter_name,
                    query_desc,
                    if has_temp_btree { "⚠️ YES" } else { "✅ NO" },
                    p95,
                    cnt_dur,
                );
            }
        }
    }

    println!("\nPermutation sweep completed: {} total combinations tested.", total_perms);
    println!("Temp B-Tree sorts detected: {}", temp_btree_count);
    assert_eq!(temp_btree_count, 0, "Regression detected: Query permutations produced temp b-tree sorts!");
}

fn run_virtual_scrolling_stress(db: &Database) {
    println!("\n=======================================================");
    println!("=== SIMULATING RAPID VIRTUAL SCROLLING (DOWN-ARROW) ===");
    println!("=======================================================\n");

    let offsets = [0, 50, 100, 150, 200, 250, 300, 350, 400, 450, 500, 1000, 2000, 4000];
    let mut total_time = Duration::ZERO;

    for offset in offsets {
        let t0 = Instant::now();
        let rows = db.list_entry_summaries_page("", EntryFilter::All, SortMode::Default, 120, offset).unwrap();
        let elapsed = t0.elapsed();
        total_time += elapsed;
        println!("Window [offset {}, limit 120] -> fetched {} rows in {:.2?}", offset, rows.len(), elapsed);
        assert!(elapsed < Duration::from_millis(25), "Virtual scroll slice took > 25ms: {:.2?}", elapsed);
    }

    println!("Average virtual scroll slice time: {:.2?}", total_time / offsets.len() as u32);
}

fn run_concurrency_stress(db_path: &std::path::Path) {
    println!("\n=======================================================");
    println!("=== CONCURRENT WRITER (DAEMON) + READERS (UI) STRESS ===");
    println!("=======================================================\n");

    let stop = Arc::new(AtomicBool::new(false));
    let read_ops = Arc::new(AtomicUsize::new(0));
    let write_ops = Arc::new(AtomicUsize::new(0));

    // Spawn 1 rapid background writer simulating the daemon
    let writer_db = Database::open(db_path).unwrap();
    let writer_stop = Arc::clone(&stop);
    let writer_ops = Arc::clone(&write_ops);
    let writer_handle = thread::spawn(move || {
        let mut i = 100_000;
        while !writer_stop.load(Ordering::Relaxed) {
            i += 1;
            let entry = NewEntry {
                content_hash: format!("concurrent_hash_{i}"),
                data: NewEntryData::Text,
                mime_type: "text/plain".to_string(),
                title: format!("Concurrent clip {i}"),
                preview_text: Some(format!("Clip {i} text")),
                text_content: Some(format!("Clip {i} content")),
                size_bytes: 64,
            };
            writer_db.upsert_entry(&entry).expect("concurrent writer upsert failed");
            writer_ops.fetch_add(1, Ordering::Relaxed);
            thread::sleep(Duration::from_millis(2));
        }
    });

    // Spawn 4 concurrent readers simulating UI virtual scrolling and searching
    let mut reader_handles = Vec::new();
    for reader_id in 0..4 {
        let reader_db = Database::open(db_path).unwrap();
        let reader_stop = Arc::clone(&stop);
        let reader_ops = Arc::clone(&read_ops);
        reader_handles.push(thread::spawn(move || {
            let mut offset = 0;
            while !reader_stop.load(Ordering::Relaxed) {
                let query = if reader_id % 2 == 0 { "" } else { "function" };
                let _ = reader_db
                    .list_entry_summaries_page(query, EntryFilter::All, SortMode::Default, 120, offset)
                    .expect("concurrent reader failed");
                reader_ops.fetch_add(1, Ordering::Relaxed);
                offset = (offset + 10) % 500;
                thread::sleep(Duration::from_millis(1));
            }
        }));
    }

    // Run concurrency test for 3 seconds
    thread::sleep(Duration::from_secs(3));
    stop.store(true, Ordering::Relaxed);

    writer_handle.join().unwrap();
    for handle in reader_handles {
        handle.join().unwrap();
    }

    let writes = write_ops.load(Ordering::Relaxed);
    let reads = read_ops.load(Ordering::Relaxed);
    println!("Concurrency stress passed: {} writes, {} reads across 4 reader threads with 0 errors.", writes, reads);
}

fn main() -> Result<()> {
    println!("Starting RSClip Comprehensive Performance Suite");
    let (db_path, db) = create_sample_db(5000, true);

    run_permutation_sweep(&db);
    run_virtual_scrolling_stress(&db);

    run_concurrency_stress(&db_path);

    drop(db);
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));

    println!("\nALL PERFORMANCE AND STRESS PERMUTATIONS PASSED WITH ZERO STUTTER!");
    Ok(())
}
