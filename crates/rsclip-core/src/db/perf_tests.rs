//! Performance regression tests for the database hot paths.
//!
//! Every UI keystroke, scroll window, and preview goes through these queries,
//! so each test pins a property that previously regressed:
//!
//! - query plans: no temp b-tree sorts or unindexed scans on browse paths;
//! - payload independence: search, filters, counts, pages, and previews must
//!   not slow down when history holds multi-megabyte entries (SQLite loads a
//!   whole TEXT value before `substr`/`LIKE`, which once made every search
//!   read 50 MB);
//! - absolute budgets: generous wall-clock ceilings measured as the best of
//!   several runs, so they only trip on real regressions, not scheduler noise.
//!
//! Set `RSCLIP_PERF_REPORT=1` and pass `--nocapture` to print every timing.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::models::{EntryFilter, NewEntry, NewEntryData, SortMode};

use super::Database;
use super::entries::{entry_count_sql, entry_page_sql};

const FILTERS: [EntryFilter; 8] = [
    EntryFilter::All,
    EntryFilter::Code,
    EntryFilter::Text,
    EntryFilter::Images,
    EntryFilter::Files,
    EntryFilter::Links,
    EntryFilter::Colors,
    EntryFilter::Pinned,
];
const SORTS: [SortMode; 5] = [
    SortMode::Default,
    SortMode::Recent,
    SortMode::Oldest,
    SortMode::Type,
    SortMode::MostUsed,
];
/// Common, prefix, code-shaped, domain, and no-match queries: the no-match
/// case forces a full scan and is the worst case for search.
const SEARCHES: [&str; 5] = ["e", "note", "fn ", "github", "zzqqxx-no-match"];
const SEARCH_FILTERS: [EntryFilter; 5] = [
    EntryFilter::All,
    EntryFilter::Code,
    EntryFilter::Text,
    EntryFilter::Images,
    EntryFilter::Links,
];

/// Entries in the synthetic history; comparable to a real 5k+ history.
const HISTORY_LEN: usize = 6_000;
/// Rows fetched per browse window and per first search page in the UI.
const WINDOW_ROWS: usize = 120;
const SEARCH_ROWS: usize = 40;

/// Ceiling for any single list/search/count query. Release builds measure
/// 0.1-7 ms on a 5k history; this leaves headroom for debug builds and busy
/// CI machines while still catching a return to 20+ ms full-payload scans.
const QUERY_BUDGET: Duration = Duration::from_millis(40);
/// Ceiling for reading one preview body on the GTK thread.
const PREVIEW_BUDGET: Duration = Duration::from_millis(2);
/// Ceiling for storing one ordinary clipboard entry (daemon path).
const UPSERT_BUDGET: Duration = Duration::from_millis(20);

/// Timestamp in the middle of the synthetic timeline; huge payloads are
/// inserted just before it so the windows around it contain them.
const MIDDLE_TS: i64 = 1_700_000_000 + (HISTORY_LEN as i64 / 2) * 60;

/// Perf tests time real work; running them concurrently would make them
/// measure each other. Each test holds this lock for its whole run.
static PERF_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn perf_lock() -> std::sync::MutexGuard<'static, ()> {
    PERF_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A temporary on-disk database removed when dropped.
struct Fixture {
    path: PathBuf,
    db: Database,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rsclip-perf-{name}-{}-{unique}.sqlite",
            std::process::id()
        ));
        let db = Database::open(&path).expect("open perf database");
        Self { path, db }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-shm"));
        let _ = std::fs::remove_file(self.path.with_extension("sqlite-wal"));
    }
}

/// A history shaped like a real one: prose, code, links, OCR'd images,
/// colors, and file lists, with spread timestamps, use counts, pins, and
/// soft-deleted rows.
fn realistic_history(name: &str) -> Fixture {
    let fixture = Fixture::new(name);
    let domains = [
        "github.com",
        "docs.rs",
        "archlinux.org",
        "youtube.com",
        "news.ycombinator.com",
    ];
    fixture
        .db
        .transaction(|db| {
            for i in 0..HISTORY_LEN {
                let hash = format!("perf-{i}");
                let entry = match i % 10 {
                    0..=3 => text(
                        &hash,
                        &format!(
                            "Meeting note {i}: follow up with the team about the release plan and the open review comments"
                        ),
                    ),
                    4 => text(
                        &hash,
                        &format!(
                            "fn handler_{i}(req: Request) -> Response {{\n    let id = req.id();\n    respond(id)\n}}"
                        ),
                    ),
                    5 => text(
                        &hash,
                        &format!("def job_{i}(items):\n    for item in items:\n        process(item)\n"),
                    ),
                    6 => text(&hash, &format!("https://{}/path/{i}", domains[i % domains.len()])),
                    7 => {
                        let mut entry = NewEntry::new(hash, "image/png".into(), format!("Image {i}"));
                        entry.data = NewEntryData::Image {
                            file_path: Some(format!("/tmp/rsclip-perf/{i}.png")),
                            thumb_path: None,
                            ocr_text: None,
                        };
                        entry
                    }
                    8 => text(&hash, &format!("#{:06x}", (i * 7919) & 0xff_ffff)),
                    _ => {
                        let uri = format!("file:///home/user/documents/report-{i}.pdf\r\n");
                        crate::classify::classify_payload("text/uri-list", hash, uri.as_bytes())?
                    }
                };
                let id = db.upsert_entry(&entry)?;
                if i % 10 == 7 && i % 3 == 0 {
                    db.save_ocr_result(id, "eng", &format!("Invoice {i} total due on receipt"))?;
                }
            }
            db.conn.execute_batch(&format!(
                r#"
                UPDATE entries
                   SET updated_at = 1700000000 + id * 60,
                       copied_at = 1700000000 + id * 60,
                       use_count = id % 13,
                       pinned = (id % 97 = 0),
                       deleted = (id % 50 = 0)
                 WHERE id <= {HISTORY_LEN};
                "#
            ))?;
            Ok(())
        })
        .expect("seed perf history");
    fixture
}

/// Build an entry through the daemon's real classifier, so previews and
/// titles are bounded exactly as in a live history.
fn text(hash: &str, body: &str) -> NewEntry {
    crate::classify::classify_text(
        "text/plain",
        hash.to_string(),
        body.to_string(),
        body.len() as i64,
    )
}

/// Add legacy multi-megabyte text entries (a real history held 41, 2.8, and
/// 1.8 MB rows) just before [`MIDDLE_TS`]. Returns the id of the largest one.
fn add_huge_payloads(db: &Database) -> i64 {
    let mut largest = 0;
    for (index, megabytes) in [24usize, 16, 8].into_iter().enumerate() {
        let body = format!(
            "Huge legacy paste {index}\n{}",
            "lorem ipsum dolor sit amet ".repeat(megabytes * 1024 * 1024 / 27)
        );
        let id = db
            .upsert_entry(&text(&format!("huge-{index}"), &body))
            .expect("insert huge payload");
        let middle = MIDDLE_TS - 1 - index as i64;
        db.conn
            .execute(
                "UPDATE entries SET updated_at = ?2, copied_at = ?2 WHERE id = ?1",
                rusqlite::params![id, middle],
            )
            .expect("move huge payload to mid-history");
        if index == 0 {
            largest = id;
        }
    }
    largest
}

/// Best of several runs: the minimum filters out preemption and cache noise.
fn best_of<T>(mut op: impl FnMut() -> T) -> Duration {
    (0..3)
        .map(|_| {
            let start = Instant::now();
            std::hint::black_box(op());
            start.elapsed()
        })
        .min()
        .expect("at least one run")
}

fn report(label: &str, elapsed: Duration) {
    if std::env::var_os("RSCLIP_PERF_REPORT").is_some() {
        eprintln!(
            "perf {label:<48} {:>8.3} ms",
            elapsed.as_secs_f64() * 1000.0
        );
    }
}

/// Every hot-path query the UI issues, labelled for failure messages.
fn hot_path_queries(db: &Database) -> Vec<(String, Duration)> {
    let mut timings = Vec::new();
    for filter in FILTERS {
        let total = db.count_entries("", filter).expect("count");
        timings.push((
            format!("count {filter:?}"),
            best_of(|| db.count_entries("", filter).expect("count")),
        ));
        for sort in SORTS {
            timings.push((
                format!("first page {filter:?} {sort:?}"),
                best_of(|| {
                    db.list_entry_summaries_page("", filter, sort, WINDOW_ROWS, 0)
                        .expect("page")
                }),
            ));
            let deepest = total.saturating_sub(WINDOW_ROWS);
            timings.push((
                format!("deepest page {filter:?} {sort:?}"),
                best_of(|| {
                    db.list_entry_summaries_page("", filter, sort, WINDOW_ROWS, deepest)
                        .expect("page")
                }),
            ));
        }
    }
    // Windows the UI loads while scrolling past the huge rows: reading a row's
    // metadata must not walk its payload (the payload column is stored last).
    let newer_than_middle = |sql: &str| -> usize {
        db.conn
            .query_row(sql, [MIDDLE_TS], |row| row.get::<_, i64>(0))
            .expect("count rows newer than middle") as usize
    };
    for (sort, offset) in [
        (
            SortMode::Default,
            newer_than_middle(
                "SELECT COUNT(*) FROM entries WHERE deleted = 0 AND (pinned = 1 OR updated_at > ?1)",
            ),
        ),
        (
            SortMode::Recent,
            newer_than_middle("SELECT COUNT(*) FROM entries WHERE deleted = 0 AND updated_at > ?1"),
        ),
    ] {
        let start = offset.saturating_sub(WINDOW_ROWS / 2);
        timings.push((
            format!("mid-history window {sort:?}"),
            best_of(|| {
                db.list_entry_summaries_page("", EntryFilter::All, sort, WINDOW_ROWS, start)
                    .expect("page")
            }),
        ));
    }
    for query in SEARCHES {
        for filter in SEARCH_FILTERS {
            timings.push((
                format!("search {query:?} {filter:?} page+count"),
                best_of(|| {
                    let page = db
                        .list_entry_summaries_page(query, filter, SortMode::Default, SEARCH_ROWS, 0)
                        .expect("search page");
                    let count = db.count_entries(query, filter).expect("search count");
                    (page, count)
                }),
            ));
        }
    }
    timings
}

#[test]
fn perf_hot_path_queries_meet_budget() {
    let _serial = perf_lock();
    let fixture = realistic_history("budget");
    let mut failures = Vec::new();
    for (label, elapsed) in hot_path_queries(&fixture.db) {
        report(&label, elapsed);
        if elapsed > QUERY_BUDGET {
            failures.push(format!("{label}: {elapsed:?} > {QUERY_BUDGET:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "queries over budget:\n{}",
        failures.join("\n")
    );
}

/// The regression that made search stutter: hot paths read full payloads, so
/// one 40 MB paste slowed every keystroke. Timing the same queries before and
/// after adding 48 MB of payloads must show no payload-proportional cost.
#[test]
fn perf_hot_paths_do_not_scale_with_payload_size() {
    let _serial = perf_lock();
    let fixture = realistic_history("payload");
    // The app's 64 MB page cache would keep freshly inserted payload pages
    // hot and hide the cost of walking them; a small cache models the cold
    // reads a real history sees. Applied to both measurements.
    fixture
        .db
        .conn
        .pragma_update(None, "cache_size", -2000)
        .expect("shrink page cache");
    let before = hot_path_queries(&fixture.db);
    let normal_text_id = 1;
    let preview_before = best_of(|| fixture.db.get_text_preview(normal_text_id, 64 * 1024 + 1));

    let huge_id = add_huge_payloads(&fixture.db);
    let after = hot_path_queries(&fixture.db);
    let preview_after = best_of(|| fixture.db.get_text_preview(huge_id, 64 * 1024 + 1));

    let mut failures = Vec::new();
    for ((label, base), (_, with_huge)) in before.iter().zip(&after) {
        report(&format!("{label} (+48 MB payloads)"), *with_huge);
        // Scanning 48 MB costs ~20 ms even in release; allow noise, not that.
        let allowed = base.mul_f64(1.5) + Duration::from_millis(2);
        if *with_huge > allowed {
            failures.push(format!(
                "{label}: {base:?} -> {with_huge:?} after adding huge payloads (allowed {allowed:?})"
            ));
        }
    }
    report("text preview normal row", preview_before);
    report("text preview 24 MB row", preview_after);
    if preview_after > PREVIEW_BUDGET {
        failures.push(format!(
            "preview of a 24 MB row took {preview_after:?} > {PREVIEW_BUDGET:?}"
        ));
    }
    assert!(
        failures.is_empty(),
        "hot paths slowed down with payload size:\n{}",
        failures.join("\n")
    );
}

/// Browse queries must walk an index in sort order (stopping after LIMIT
/// rows), never sort the whole history in a temp b-tree. Checked on the exact
/// SQL the app runs, for every filter and sort.
#[test]
fn perf_browse_query_plans_avoid_sorts_and_table_scans() {
    let _serial = perf_lock();
    let fixture = realistic_history("plans");
    let explain = |sql: &str, params: &[&dyn rusqlite::ToSql]| -> String {
        let mut stmt = fixture
            .db
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("explain");
        stmt.query_map(params, |row| row.get::<_, String>(3))
            .expect("plan rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("plan")
            .join("\n")
    };

    let mut failures = Vec::new();
    for filter in FILTERS {
        for sort in SORTS {
            let plan = explain(&entry_page_sql(filter, sort, false, false), &[&120, &0]);
            if plan.contains("TEMP B-TREE") {
                failures.push(format!(
                    "page {filter:?} {sort:?} sorts in a temp b-tree:\n{plan}"
                ));
            }
            if plan
                .lines()
                .any(|line| line.trim_start_matches(['|', '-', '`', ' ']) == "SCAN e")
            {
                failures.push(format!(
                    "page {filter:?} {sort:?} scans without an index:\n{plan}"
                ));
            }
            // Search scans rows by nature, but must still stream in sort order.
            let plan = explain(
                &entry_page_sql(filter, sort, true, false),
                &[&"%x%", &40, &0],
            );
            if plan.contains("TEMP B-TREE") {
                failures.push(format!(
                    "search {filter:?} {sort:?} sorts in a temp b-tree:\n{plan}"
                ));
            }
        }
        let plan = explain(&entry_count_sql(filter, false), &[]);
        if plan
            .lines()
            .any(|line| line.trim_start_matches(['|', '-', '`', ' ']) == "SCAN e")
        {
            failures.push(format!("count {filter:?} scans without an index:\n{plan}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The daemon stores every copy synchronously; a slow upsert (for example
/// detecting the language of a whole multi-megabyte payload) delays the
/// entry appearing in the overlay.
#[test]
fn perf_storing_entries_meets_budget() {
    let _serial = perf_lock();
    let fixture = realistic_history("upsert");
    let mut counter = 0;
    let ordinary = best_of(|| {
        counter += 1;
        let body = format!("fn fresh_{counter}() {{\n    println!(\"copied\");\n}}");
        fixture
            .db
            .upsert_entry(&text(&format!("fresh-{counter}"), &body))
            .expect("upsert")
    });
    report("upsert ordinary entry", ordinary);
    assert!(
        ordinary < UPSERT_BUDGET,
        "ordinary upsert took {ordinary:?}"
    );

    let entry = text(
        "large-code",
        &"let value = compute();\n".repeat(4 * 1024 * 1024 / 23),
    );
    let start = Instant::now();
    fixture.db.upsert_entry(&entry).expect("upsert large");
    let large = start.elapsed();
    report("upsert 4 MB code entry", large);
    assert!(
        large < Duration::from_millis(50),
        "storing a 4 MB entry took {large:?}; language detection must stay bounded"
    );
}
