//! Headless regression test for GTK-thread stalls.
//!
//! Builds the real overlay window against a private headless weston, seeds a
//! synthetic history with the payload shapes that used to freeze the UI (a
//! 64 KiB single-line base64 blob, large logs, a multi-megabyte legacy row,
//! a large image), then drives the real handlers: typing into the search
//! entry, key navigation, previews, copying, and favicon refreshes. Every GLib
//! main-loop dispatch is timed; one over the budget is a visible stutter.
//!
//! The test starts its own compositor, so it never touches the desktop
//! session. It is skipped when `weston` is not installed, unless
//! `RSCLIP_REQUIRE_HEADLESS_UI=1` (set in CI) turns that into a failure.
//! `RSCLIP_UI_DISPATCH_BUDGET_MS` overrides the per-dispatch budget.
#![allow(dead_code, unused_imports)]

#[path = "../src/actions/mod.rs"]
mod actions;
#[path = "../src/app.rs"]
mod app;
#[path = "../src/cli.rs"]
mod cli;
#[path = "../src/components/mod.rs"]
mod components;
#[path = "../src/config_reload.rs"]
mod config_reload;
#[path = "../src/dialogs/mod.rs"]
mod dialogs;
#[path = "../src/events.rs"]
mod events;
#[path = "../src/highlight.rs"]
mod highlight;
#[path = "../src/notify.rs"]
mod notify;
#[path = "../src/state.rs"]
mod state;
#[path = "../src/style.rs"]
mod style;
#[path = "../src/window.rs"]
mod window;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gio::prelude::*;
use gtk4 as gtk;
use gtk4::prelude::*;
use rsclip_core::Database;
use rsclip_core::config::RsclipPaths;
use rsclip_core::models::{EntryData, NewEntryData};

use components::preview::{
    FULL_PREVIEW_TRUNCATED_NOTICE, MAX_PREVIEW_BYTES, MAX_PREVIEW_LINE_BYTES, MAX_PREVIEW_LINES,
    PREVIEW_LINE_TRUNCATED_MARKER,
};
use state::{AfterCopy, AppState};

/// Longest acceptable single main-loop dispatch. Stutters this fixes were
/// 375 ms to 1.3 s; after the fix the worst dispatch is ~20 ms on a laptop,
/// so the default leaves headroom for slow CI runners.
const DEFAULT_BUDGET_MS: f64 = 100.0;
const WAYLAND_SOCKET: &str = "rsclip-ui-test";

struct Weston(Child);

impl Drop for Weston {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_weston(runtime_dir: &Path, log: &Path) -> Option<Weston> {
    let child = Command::new("weston")
        .args([
            "--backend=headless",
            "--renderer=pixman",
            "--width=1280",
            "--height=800",
            "--idle-time=0",
            &format!("--socket={WAYLAND_SOCKET}"),
        ])
        .env("XDG_RUNTIME_DIR", runtime_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(log).ok()?)
        .spawn()
        .ok()?;
    let weston = Weston(child);
    let socket = runtime_dir.join(WAYLAND_SOCKET);
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if socket.exists() {
            return Some(weston);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Run the main loop for `span`; returns the longest single dispatch (ms).
fn pump(span: Duration) -> f64 {
    let context = gtk::glib::MainContext::default();
    let end = Instant::now() + span;
    let mut worst = 0.0_f64;
    while Instant::now() < end {
        let started = Instant::now();
        if context.iteration(false) {
            worst = worst.max(ms(started.elapsed()));
        } else {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    worst
}

/// Pump until the main loop has been idle for a while.
fn settle() {
    for _ in 0..50 {
        if pump(Duration::from_millis(100)) < 1.0 {
            break;
        }
    }
}

/// Type `query` one character at a time at `cadence`; returns the worst dispatch.
fn type_query(state: &Rc<AppState>, query: &str, cadence: Duration) -> f64 {
    state.search_entry.set_text("");
    settle();
    let mut worst = 0.0_f64;
    for ch in query.chars() {
        let started = Instant::now();
        let mut position = state.search_entry.text().chars().count() as i32;
        state
            .search_entry
            .insert_text(&ch.to_string(), &mut position);
        worst = worst.max(ms(started.elapsed()));
        worst = worst.max(pump(cadence));
    }
    worst.max(pump(Duration::from_millis(500)))
}

/// The visible preview text must stay within the layout bounds.
fn assert_preview_bounded(state: &Rc<AppState>, case: &str) {
    let buffers: [&gtk::TextBuffer; 2] = [
        &state.channels.text_buffer,
        state.channels.code_buffer.upcast_ref(),
    ];
    for buffer in buffers {
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        let body = text
            .strip_suffix(FULL_PREVIEW_TRUNCATED_NOTICE)
            .unwrap_or(&text);
        let longest = body.split('\n').map(str::len).max().unwrap_or(0);
        assert!(
            longest <= MAX_PREVIEW_LINE_BYTES + PREVIEW_LINE_TRUNCATED_MARKER.len(),
            "{case}: preview shows a {longest}-byte line"
        );
        assert!(
            body.split('\n').count() <= MAX_PREVIEW_LINES,
            "{case}: preview shows {} lines",
            body.split('\n').count()
        );
        assert!(
            body.len() <= MAX_PREVIEW_BYTES + PREVIEW_LINE_TRUNCATED_MARKER.len(),
            "{case}: preview shows {} bytes",
            body.len()
        );
    }
}

/// A full (non-summary) text entry, so the preview uses `text` as is.
fn synthetic_text_entry(id: i64, text: &str) -> rsclip_core::models::ClipboardEntry {
    rsclip_core::models::ClipboardEntry {
        id,
        content_hash: String::new(),
        kind: rsclip_core::models::EntryKind::Text,
        mime_type: "text/plain".into(),
        title: "snippet".into(),
        preview_text: Some(text.into()),
        text_content: Some(text.into()),
        pinned: false,
        copied_at: 0,
        updated_at: 0,
        last_used_at: None,
        use_count: 0,
        size_bytes: text.len() as i64,
        data: EntryData::Text,
    }
}

fn insert_text(db: &Database, hash: &str, text: String) -> i64 {
    let entry = rsclip_core::classify::classify_payload("text/plain", hash.into(), text.as_bytes())
        .unwrap();
    db.upsert_entry(&entry).unwrap()
}

/// A noisy PNG, so the file is several megabytes like a real screenshot.
fn write_large_png(path: &Path) {
    let (width, height) = (2400, 1600);
    let pixbuf =
        gdk_pixbuf::Pixbuf::new(gdk_pixbuf::Colorspace::Rgb, false, 8, width, height).unwrap();
    let mut seed = 0x2545_f491_u32;
    let rowstride = pixbuf.rowstride() as usize;
    // SAFETY: the pixbuf is fresh and not shared.
    let pixels = unsafe { pixbuf.pixels() };
    for y in 0..height as usize {
        for byte in &mut pixels[y * rowstride..y * rowstride + width as usize * 3] {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *byte = seed as u8;
        }
    }
    pixbuf.savev(path, "png", &[]).unwrap();
}

struct Seeded {
    huge_id: i64,
    huge_len: usize,
    image_id: i64,
}

/// A history with filler rows plus every payload shape that once stalled
/// the UI. The pathological rows are inserted last so searches select them.
fn seed_history(paths: &RsclipPaths) -> Seeded {
    let db = Database::open(&paths.db_path).unwrap();
    for index in 0..1_500 {
        insert_text(
            &db,
            &format!("filler-{index}"),
            format!("note {index} alpha beta gamma"),
        );
    }
    for index in 0..100 {
        insert_text(
            &db,
            &format!("link-{index}"),
            format!("https://site{index}.example.com/page"),
        );
    }

    std::fs::create_dir_all(&paths.image_dir).unwrap();
    let image_path = paths.image_dir.join("large.png");
    write_large_png(&image_path);
    let size = std::fs::metadata(&image_path).unwrap().len() as i64;
    let mut image = rsclip_core::models::NewEntry::new(
        "image-large".into(),
        "image/png".into(),
        "qximage screenshot".into(),
    );
    image.size_bytes = size;
    image.data = NewEntryData::Image {
        file_path: Some(image_path.to_string_lossy().into_owned()),
        thumb_path: None,
        ocr_text: None,
    };
    let image_id = db.upsert_entry(&image).unwrap();

    let code = "fn qxcode() {\n    let value = compute(1, 2);\n    println!(\"{value}\");\n}\n"
        .repeat(800);
    insert_text(&db, "code", code);
    let log = (0..6_000)
        .map(|line| format!("2026-09-30T12:00:{line:05} INFO qxlog request {line} served in 3ms\n"))
        .collect::<String>();
    insert_text(&db, "log", log);
    let json = format!(
        "{{\"qxjson\": [\n{}]}}",
        "  {\"key\": \"value\", \"number\": 12345, \"flag\": true},\n".repeat(1_300)
    );
    insert_text(&db, "json", json);
    insert_text(
        &db,
        "base64",
        format!("qxbase{}", "QUJDRA==".repeat(8 * 1024)),
    );
    let huge = format!("qxhuge {}\n", "legacy payload line ".repeat(8)).repeat(30_000);
    let huge_len = huge.len();
    let huge_id = insert_text(&db, "huge", huge);
    Seeded {
        huge_id,
        huge_len,
        image_id,
    }
}

fn main() {
    perf_gtk_thread_stays_responsive();
}

fn perf_gtk_thread_stays_responsive() {
    let required = std::env::var_os("RSCLIP_REQUIRE_HEADLESS_UI").is_some_and(|value| value == "1");
    let budget = std::env::var("RSCLIP_UI_DISPATCH_BUDGET_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_BUDGET_MS);

    let root = std::env::temp_dir().join(format!("rsclip-ui-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let runtime_dir = root.join("runtime");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    std::fs::set_permissions(
        &runtime_dir,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();

    // Headless weston has no seat for the real wl-copy; this stand-in keeps
    // what it is given, so the test can check the whole payload arrived.
    let bin_dir = root.join("bin");
    let clipboard_file = root.join("clipboard");
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::write(
        bin_dir.join("wl-copy"),
        format!("#!/bin/sh\ncat > '{}'\n", clipboard_file.display()),
    )
    .unwrap();
    std::fs::set_permissions(
        bin_dir.join("wl-copy"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    let Some(_weston) = start_weston(&runtime_dir, &root.join("weston.log")) else {
        assert!(
            !required,
            "RSCLIP_REQUIRE_HEADLESS_UI=1 but headless weston did not start (see {})",
            root.join("weston.log").display()
        );
        eprintln!("skipping: headless weston is not available");
        return;
    };

    // SAFETY: this binary runs one scenario on its main thread, and sets the
    // environment before GTK or any other thread reads it.
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", WAYLAND_SOCKET);
        std::env::set_var("XDG_RUNTIME_DIR", &runtime_dir);
        std::env::set_var("XDG_STATE_HOME", root.join("state"));
        std::env::set_var("XDG_DATA_HOME", root.join("data"));
        std::env::set_var("XDG_CONFIG_HOME", root.join("config"));
        std::env::set_var("XDG_CACHE_HOME", root.join("cache"));
        std::env::set_var("GDK_BACKEND", "wayland");
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs = vec![bin_dir.clone()];
        dirs.extend(std::env::split_paths(&path));
        std::env::set_var("PATH", std::env::join_paths(dirs).unwrap());
        std::env::remove_var("DISPLAY");
        if std::env::var_os("GSK_RENDERER").is_none() {
            std::env::set_var("GSK_RENDERER", "cairo");
        }
    }

    let paths = RsclipPaths::discover().unwrap();
    paths.ensure().unwrap();
    let seeded = seed_history(&paths);

    gtk::init().expect("GTK connects to the headless compositor");
    // weston has no layer-shell, so the overlay is a plain toplevel and
    // gtk4-layer-shell warns on every show/hide; keep other warnings.
    gtk::glib::log_set_default_handler(|domain, level, message| {
        if !message.contains("is not a layer surface") {
            gtk::glib::log_default_handler(domain, level, Some(message));
        }
    });
    let app = gtk::Application::builder()
        .application_id("io.github.radhey.rsclip.perftest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(gio::Cancellable::NONE).unwrap();
    let runtime = window::build_ui(&app).unwrap();
    let state = Rc::clone(&runtime.state);
    runtime.show_reset().unwrap();
    pump(Duration::from_millis(1_500));
    settle();

    let mut results: Vec<(String, f64)> = Vec::new();

    // Typing: every keystroke auto-selects the top hit and renders its preview.
    for query in [
        "qxbase", "qxjson", "qxlog", "qxcode", "qxhuge", "a", "note 1",
    ] {
        for cadence in [150, 40] {
            let worst = type_query(&state, query, Duration::from_millis(cadence));
            assert_preview_bounded(&state, &format!("typing {query:?}"));
            results.push((format!("type {query:?} @{cadence}ms/char"), worst));
        }
    }

    // Selecting each row once: previews of every kind, including the image.
    state.search_entry.set_text("");
    settle();
    let mut worst_nav = 0.0_f64;
    for _ in 0..40 {
        let started = Instant::now();
        actions::selection::move_selection(&state, 1);
        worst_nav = worst_nav.max(ms(started.elapsed()));
        worst_nav = worst_nav.max(pump(Duration::from_millis(30)));
        assert_preview_bounded(&state, "key navigation");
    }
    results.push(("key navigation x40".into(), worst_nav));

    let image = state.db.get_entry(seeded.image_id).unwrap().unwrap();
    state::invalidate_preview_cache(&state);
    let started = Instant::now();
    components::preview::render_preview(&state, &image);
    let sync = ms(started.elapsed());
    results.push((
        "image preview (cold)".into(),
        sync.max(pump(Duration::from_millis(1_000))),
    ));

    // Arrowing through code in several languages: after each language's
    // first use, switching back to it must not recompile its highlighting.
    let snippets = [
        "#!/usr/bin/env python3\nimport os\n\ndef main():\n    print(os.getcwd())\n",
        "# Release notes\n\n- faster previews\n- **bold** and `code`\n\n```sh\ncargo test\n```\n",
        "fn main() {\n    let value = compute(1, 2);\n    println!(\"{value}\");\n}\n",
        "#include <stdio.h>\n\nint main(void) {\n    printf(\"hi\\n\");\n    return 0;\n}\n",
        "<!DOCTYPE html>\n<html><head><style>p { color: red; }</style></head>\n<body><p>hi</p><script>let x = 1;</script></body></html>\n",
    ];
    let code_entries: Vec<_> = snippets
        .iter()
        .enumerate()
        .map(|(index, text)| synthetic_text_entry(-1 - index as i64, text))
        .collect();
    let mut round_totals = Vec::new();
    let mut worst_repeat = 0.0_f64;
    for round in 0..3 {
        let mut total = 0.0;
        for entry in &code_entries {
            state::invalidate_preview_cache(&state);
            let started = Instant::now();
            components::preview::render_preview(&state, entry);
            let sync = ms(started.elapsed());
            total += sync;
            let worst = sync.max(pump(Duration::from_millis(60)));
            if round > 0 {
                worst_repeat = worst_repeat.max(worst);
            }
        }
        round_totals.push(total);
    }
    results.push(("code preview, language switch".into(), worst_repeat));
    let repeat = round_totals[1].max(round_totals[2]);
    assert!(
        repeat <= (round_totals[0] * 0.5).max(10.0),
        "switching back to a highlighted language recompiled it: first round {:.1} ms, later rounds {:?} ms",
        round_totals[0],
        &round_totals[1..]
    );

    // Copying the multi-megabyte row: the read and wl-copy run on the copy
    // worker, and the whole payload must reach the clipboard.
    type_query(&state, "qxhuge", Duration::from_millis(100));
    state.footer.set_text("");
    let started = Instant::now();
    actions::clipboard::queue_entry_copy(&state, seeded.huge_id, AfterCopy::Report).unwrap();
    let mut worst_copy = ms(started.elapsed());
    let deadline = Instant::now() + Duration::from_secs(10);
    while state.footer.text().is_empty() && Instant::now() < deadline {
        worst_copy = worst_copy.max(pump(Duration::from_millis(20)));
    }
    assert_eq!(
        state.footer.text(),
        "Copied selected entry",
        "copying the 5 MB entry did not succeed"
    );
    let copied = std::fs::metadata(&clipboard_file).map_or(0, |meta| meta.len() as usize);
    assert_eq!(copied, seeded.huge_len, "wl-copy got a partial payload");
    results.push((
        "copy 5 MB entry".into(),
        worst_copy.max(pump(Duration::from_millis(300))),
    ));

    // A paste only fires for the newest clipboard write in the same overlay
    // session: a later copy, or closing and reopening, cancels it.
    state.auto_paste.set(false);
    let pump_until = |done: &dyn Fn() -> bool| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done() && Instant::now() < deadline {
            pump(Duration::from_millis(20));
        }
    };
    state.footer.set_text("");
    actions::clipboard::queue_entry_copy(&state, seeded.huge_id, AfterCopy::Paste).unwrap();
    actions::clipboard::queue_entry_copy(&state, seeded.huge_id, AfterCopy::Report).unwrap();
    // Copies finish in order, so the report arrives after the paste.
    pump_until(&|| !state.footer.text().is_empty());
    assert!(
        runtime.window.is_visible(),
        "a paste fired after a later copy replaced its clipboard value"
    );
    actions::clipboard::queue_entry_copy(&state, seeded.huge_id, AfterCopy::Paste).unwrap();
    runtime.hide();
    runtime.show_reset().unwrap();
    // No later request to wait on without superseding it; the copy takes
    // well under this.
    pump(Duration::from_secs(2));
    assert!(
        runtime.window.is_visible(),
        "a paste from a closed overlay session closed the reopened one"
    );
    actions::clipboard::queue_entry_copy(&state, seeded.huge_id, AfterCopy::Paste).unwrap();
    pump_until(&|| !runtime.window.is_visible());
    assert!(
        !runtime.window.is_visible(),
        "a current paste did not close the overlay"
    );
    runtime.show_reset().unwrap();
    settle();

    // Favicon fetch landing while the overlay is open.
    type_query(&state, "site", Duration::from_millis(100));
    let started = Instant::now();
    actions::refresh::rerender_link_rows(&state);
    let sync = ms(started.elapsed());
    results.push((
        "favicon refresh".into(),
        sync.max(pump(Duration::from_millis(300))),
    ));

    println!("\nworst GTK main-loop dispatch per scenario (budget {budget:.0} ms)");
    for (name, worst) in &results {
        println!("  {name:<32} {worst:8.2} ms");
    }
    let over: Vec<_> = results
        .iter()
        .filter(|(_, worst)| *worst > budget)
        .collect();
    assert!(
        over.is_empty(),
        "GTK thread stalled past {budget:.0} ms: {over:?}"
    );

    drop(runtime);
    let _ = std::fs::remove_dir_all(&root);
}
