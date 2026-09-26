use std::rc::Rc;

use gtk::gdk;
use gtk4 as gtk;
// sourceview5's prelude re-exports gtk4's, which covers gtk trait methods here.
use rsclip_core::colors::parse_color;
use rsclip_core::files::{URI_LIST_PREVIEW_MAX_FILES, parse_uri_list_bounded};
use rsclip_core::format::masked_secret;
use rsclip_core::models::{ClipboardEntry, EntryData, SecretEntry};
use rsclip_core::syntax::CodeLanguage;
use sourceview5::prelude::*;

use crate::components::details::{render_details, render_secret_details};
use crate::components::labels::{muted_label, section_label};
use crate::state::AppState;

/// UI-side safety net for previews. Capped at 64 KiB so large text entries
/// layout smoothly in `TextView` without blocking the GTK main thread.
pub(crate) const MAX_FULL_PREVIEW_BYTES: usize = 64 * 1024;
/// Payloads larger than this render as plain monospace (no syntax coloring).
/// gtksourceview's context parse costs roughly 1 ms per KiB, so a 64 KiB code
/// row cost ~57 ms per keypress; content beyond the bound is still shown and
/// copy always delivers the full text.
pub(crate) const MAX_HIGHLIGHT_BYTES: usize = 8 * 1024;
pub(crate) const FULL_PREVIEW_TRUNCATED_NOTICE: &str =
    "\n\n[Preview truncated — copy the entry for full content]";
pub(crate) const BINARY_PREVIEW_NOTICE: &str = "[Binary data — copy the entry for full content]";

pub(crate) struct PreviewPanel {
    pub(crate) shell: gtk::Box,
    pub(crate) preview: gtk::Box,
    pub(crate) details: gtk::Box,
    pub(crate) channels: crate::state::PreviewChannels,
}

pub(crate) fn build_panel() -> PreviewPanel {
    let shell = gtk::Box::new(gtk::Orientation::Vertical, 8);
    shell.set_vexpand(true);
    shell.add_css_class("preview-pane");

    let preview = gtk::Box::new(gtk::Orientation::Vertical, 8);
    preview.set_vexpand(true);
    let preview_scroller = gtk::ScrolledWindow::builder()
        .min_content_width(150)
        .min_content_height(64)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&preview)
        .build();
    shell.append(&preview_scroller);

    let details = gtk::Box::new(gtk::Orientation::Vertical, 4);
    details.add_css_class("details-panel");
    details.set_hexpand(true);
    shell.append(&details);

    // Plain-text preview channel: one TextView whose buffer is re-filled per
    // selection instead of a fresh TextView per selection.
    let text_buffer = gtk::TextBuffer::new(None);
    let text_view = gtk::TextView::with_buffer(&text_buffer);
    text_view.add_css_class("preview-text");
    text_view.set_editable(false);
    text_view.set_cursor_visible(false);
    text_view.set_wrap_mode(gtk::WrapMode::WordChar);
    text_view.set_monospace(true);
    text_view.set_vexpand(true);
    text_view.set_left_margin(8);
    text_view.set_right_margin(8);
    let text_scroller = gtk::ScrolledWindow::builder()
        .min_content_height(80)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .propagate_natural_height(false)
        .child(&text_view)
        .build();

    // Code preview channel: persistent sourceview View/Buffer pair. Reuse
    // keeps the highlighting engine's per-buffer analysis cache warm.
    let code_buffer = crate::highlight::setup_source_buffer("", CodeLanguage::Rust);
    let code_view = crate::highlight::create_source_view(&code_buffer);
    let code_scroller = gtk::ScrolledWindow::builder()
        .min_content_height(80)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .propagate_natural_height(false)
        .child(&code_view)
        .build();

    PreviewPanel {
        shell,
        preview,
        details,
        channels: crate::state::PreviewChannels {
            text: text_scroller,
            text_buffer,
            code: code_scroller,
            code_buffer,
        },
    }
}

pub(crate) fn render_secret_preview(state: &Rc<AppState>, secret: &SecretEntry) {
    if state.currently_previewed_secret_id.get() == Some(secret.id) {
        return;
    }
    state.currently_previewed_secret_id.set(Some(secret.id));
    state.currently_previewed_entry_id.set(None);

    crate::components::clear_box(&state.preview);
    crate::components::clear_box(&state.details);
    state.ocr_button.set_opacity(0.0);
    state.ocr_button.set_sensitive(false);

    state
        .channels
        .text_buffer
        .set_text(&masked_secret(&secret.value));
    state.channels.text.vadjustment().set_value(0.0);
    state.preview.append(&state.channels.text);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);

    let copy_button = gtk::Button::with_label("Copy");
    {
        let state = Rc::clone(state);
        let secret = secret.clone();
        copy_button.connect_clicked(move |_| {
            if let Err(err) = crate::actions::clipboard::copy_secret(&state, &secret) {
                crate::actions::set_footer(&state, &format!("Copy failed: {err:#}"));
            } else {
                crate::actions::set_footer(&state, "Copied secret");
            }
        });
    }
    actions.append(&copy_button);

    let reveal_button = gtk::Button::with_label("Reveal");
    {
        let buffer = state.channels.text_buffer.clone();
        let value = secret.value.clone();
        let masked = masked_secret(&secret.value);
        reveal_button.connect_clicked(move |button| {
            if button.label().as_deref() == Some("Reveal") {
                buffer.set_text(&value);
                button.set_label("Hide");
            } else {
                buffer.set_text(&masked);
                button.set_label("Reveal");
            }
        });
    }
    actions.append(&reveal_button);

    let rename_button = gtk::Button::with_label("Rename");
    {
        let state = Rc::clone(state);
        let window = state
            .list
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok());
        rename_button.connect_clicked(move |_| {
            if let Some(window) = window.as_ref() {
                crate::actions::secrets::rename_current_secret_dialog(&state, window);
            }
        });
    }
    actions.append(&rename_button);

    let delete_button = gtk::Button::with_label("Delete");
    delete_button.add_css_class("destructive-button");
    {
        let state = Rc::clone(state);
        delete_button.connect_clicked(move |_| {
            if let Err(err) = crate::actions::secrets::delete_current(&state) {
                crate::actions::set_footer(&state, &format!("Delete failed: {err:#}"));
            } else {
                crate::actions::set_footer(&state, "Deleted secret");
            }
        });
    }
    actions.append(&delete_button);

    state.preview.append(&actions);
    render_secret_details(&state.details, secret);
}

pub(crate) fn render_preview(state: &Rc<AppState>, entry: &ClipboardEntry) {
    if state.currently_previewed_entry_id.get() == Some(entry.id) {
        return;
    }
    state.currently_previewed_entry_id.set(Some(entry.id));
    if state.currently_previewed_secret_id.take().is_some() {
        state.channels.text_buffer.set_text("");
    }

    let generation = state.preview_generation.get().wrapping_add(1);
    state.preview_generation.set(generation);

    rsclip_core::profiler::begin_phase("render_preview");
    crate::components::clear_box(&state.preview);
    crate::components::clear_box(&state.details);
    let is_image = matches!(&entry.data, EntryData::Image { .. });
    let can_ocr = is_image && state.ocr_enabled.get();
    state
        .ocr_button
        .set_opacity(if can_ocr { 1.0 } else { 0.0 });
    state.ocr_button.set_sensitive(can_ocr);

    // Summary rows omit text payloads; small ones re-read inline (sub-
    // millisecond). A summary whose payload exceeds the preview cap would
    // block the keypress with a multi-millisecond read of a huge row: render
    // the snippet now and let the deferred upgrade do the single full read.
    let (full, defer_full_read) = match &entry.data {
        EntryData::Text | EntryData::Unknown | EntryData::File { .. } => {
            let huge_summary =
                entry.text_content.is_none() && entry.size_bytes > MAX_FULL_PREVIEW_BYTES as i64;
            if huge_summary {
                (entry.clone(), true)
            } else {
                (full_entry_for_preview(state, entry), false)
            }
        }
        _ => (entry.clone(), false),
    };

    match &full.data {
        EntryData::Image { .. } => render_image_preview(&state.preview, &full),
        EntryData::Color { value, .. } => render_color_preview(state, value),
        EntryData::Link { url, .. } => {
            render_text_preview_state(state, Some(url));
        }
        EntryData::File { .. } => render_file_preview(state, &full),
        EntryData::Text | EntryData::Unknown => {
            let content = full
                .text_content
                .as_deref()
                .or(full.preview_text.as_deref());
            if content.is_none() && matches!(full.data, EntryData::Unknown) {
                render_text_preview_state(state, Some(BINARY_PREVIEW_NOTICE));
            } else {
                render_text_or_code_preview(state, content);
            }
        }
    }

    if let EntryData::Image {
        ocr_text: Some(ocr),
        ..
    } = &full.data
        && !ocr.is_empty()
    {
        render_ocr_header(state, ocr);
        render_text_preview_state(state, Some(ocr));
    }

    if defer_full_read {
        schedule_preview_upgrade(state, full.clone(), generation);
    }

    render_details(&state.details, &full);
    rsclip_core::profiler::end_phase("render_preview");
    if rsclip_core::profiler::enabled() {
        rsclip_core::profiler::print_report();
        rsclip_core::profiler::reset();
    }
}

pub(crate) fn clear_preview_state(state: &Rc<AppState>) {
    crate::state::invalidate_preview_cache(state);
    crate::components::clear_box(&state.preview);
    crate::components::clear_box(&state.details);
    state.channels.text_buffer.set_text("");
    state.channels.code_buffer.set_text("");
    state.ocr_button.set_opacity(0.0);
    state.ocr_button.set_sensitive(false);
}

fn render_image_preview(container: &gtk::Box, entry: &ClipboardEntry) {
    rsclip_core::profiler::begin_phase("render_image_preview");
    if let EntryData::Image { file_path, .. } = &entry.data {
        let file = gio::File::for_path(file_path);
        if let Ok(texture) = gdk::Texture::from_file(&file) {
            let ratio = (texture.width() as f32 / texture.height().max(1) as f32).clamp(0.2, 8.0);
            let frame = gtk::AspectFrame::new(0.5, 0.5, ratio, false);
            frame.set_hexpand(true);
            frame.set_vexpand(true);

            let picture = gtk::Picture::for_paintable(&texture);
            picture.set_content_fit(gtk::ContentFit::Contain);
            picture.set_can_shrink(true);
            picture.set_hexpand(true);
            picture.set_vexpand(true);
            frame.set_child(Some(&picture));
            container.append(&frame);
        } else {
            container.append(&muted_label("Image preview is unavailable"));
        }
    } else {
        container.append(&muted_label("Image file is missing"));
    }
    rsclip_core::profiler::end_phase("render_image_preview");
}

fn render_file_preview(state: &Rc<AppState>, entry: &ClipboardEntry) {
    rsclip_core::profiler::begin_phase("render_file_preview");
    let Some(payload) = entry.text_content.as_deref() else {
        state
            .preview
            .append(&muted_label("File list is unavailable"));
        rsclip_core::profiler::end_phase("render_file_preview");
        return;
    };
    // Bound parsing, allocation, and `exists()` stats before touching the
    // payload: legacy rows can hold thousands of URIs and this runs on the
    // GTK thread.
    let bounded =
        parse_uri_list_bounded(payload, URI_LIST_PREVIEW_MAX_FILES, MAX_FULL_PREVIEW_BYTES);
    if bounded.files.is_empty() {
        render_text_preview_state(state, Some(payload));
        rsclip_core::profiler::end_phase("render_file_preview");
        return;
    }

    let files = &bounded.files;
    let paths = files
        .iter()
        .map(|file| file.path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let missing_count = files.iter().filter(|file| !file.path.exists()).count();

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_hexpand(true);

    let title = section_label("Files");
    title.set_hexpand(true);
    header.append(&title);

    let mut status = if bounded.truncated {
        format!("{}+ files", files.len())
    } else {
        file_count_label(files.len())
    };
    if missing_count > 0 {
        status.push_str(&format!(", {missing_count} missing"));
    }
    header.append(&muted_label(&status));

    let copy = gtk::Button::with_label("Copy paths");
    copy.add_css_class("primary-button");
    {
        let state = Rc::clone(state);
        let paths = paths.clone();
        copy.connect_clicked(move |_| {
            if let Err(err) = crate::actions::clipboard::copy_text(&paths) {
                crate::actions::set_footer(&state, &format!("Copy paths failed: {err:#}"));
            } else {
                crate::actions::set_footer(&state, "Copied paths");
            }
        });
    }
    header.append(&copy);
    state.preview.append(&header);

    // "Copy paths" only copies the shown subset, so label partial results.
    if bounded.truncated {
        state.preview.append(&muted_label(
            "[File list truncated — copy the entry for the full list]",
        ));
    }

    render_text_preview_state(state, Some(&paths));
    rsclip_core::profiler::end_phase("render_file_preview");
}

fn file_count_label(count: usize) -> String {
    if count == 1 {
        "1 file".to_string()
    } else {
        format!("{count} files")
    }
}

fn render_color_preview(state: &Rc<AppState>, hex: &str) {
    let frame = gtk::AspectFrame::new(0.5, 0.5, 16.0 / 9.0, false);
    frame.set_hexpand(true);
    frame.set_vexpand(true);

    let swatch = gtk::DrawingArea::new();
    swatch.add_css_class("color-swatch");
    swatch.set_hexpand(true);
    swatch.set_vexpand(true);
    let color = parse_color(hex)
        .map(|color| {
            (
                f64::from(color.rgb.0) / 255.0,
                f64::from(color.rgb.1) / 255.0,
                f64::from(color.rgb.2) / 255.0,
            )
        })
        .unwrap_or((0.2, 0.2, 0.2));
    swatch.set_draw_func(move |_, cr, width, height| {
        if width <= 0 || height <= 0 {
            return;
        }
        cr.set_source_rgb(color.0, color.1, color.2);
        cr.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
        let _ = cr.fill();
    });
    frame.set_child(Some(&swatch));
    state.preview.append(&frame);
    render_text_preview_state(state, Some(hex));
}

fn render_ocr_header(state: &Rc<AppState>, ocr: &str) {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_hexpand(true);

    let title = section_label("OCR");
    title.set_hexpand(true);
    header.append(&title);

    let copy = gtk::Button::with_label("Copy OCR");
    copy.add_css_class("primary-button");
    {
        let state = Rc::clone(state);
        let ocr = ocr.to_string();
        copy.connect_clicked(move |_| {
            if let Err(err) = crate::actions::clipboard::copy_text(&ocr) {
                crate::actions::set_footer(&state, &format!("Copy OCR failed: {err:#}"));
            } else {
                crate::actions::set_footer(&state, "Copied OCR text");
            }
        });
    }
    header.append(&copy);
    state.preview.append(&header);
}

/// Re-read one entry from SQLite so previews can show the full payload.
///
/// Falls back to the summary row when the entry vanished or the read failed.
fn full_entry_for_preview(state: &Rc<AppState>, entry: &ClipboardEntry) -> ClipboardEntry {
    if entry.text_content.is_some() {
        return entry.clone();
    }

    rsclip_core::profiler::begin_phase("query_full_entry");
    // The preview pane truncates at MAX_FULL_PREVIEW_BYTES anyway; cap the SQL
    // read to match so a legacy multi-megabyte payload cannot stall the GTK
    // thread. Copying still uses the full row via Database::get_entry.
    let result = state
        .db
        .get_entry_preview(entry.id, MAX_FULL_PREVIEW_BYTES + 1)
        .ok()
        .flatten()
        .unwrap_or_else(|| entry.clone());
    rsclip_core::profiler::end_phase("query_full_entry");
    result
}

fn is_binary_payload(text: &str) -> bool {
    text.as_bytes().contains(&0)
}

fn sanitize_preview_text(text: &str) -> std::borrow::Cow<'_, str> {
    let preview = bounded_full_preview(text);
    if preview.contains('\0') {
        std::borrow::Cow::Owned(preview.replace('\0', " "))
    } else {
        preview
    }
}

/// Upgrade a just-rendered summary preview to its full content after the fact.
///
/// Rows whose payload exceeds the preview cap would block the keypress with a
/// multi-millisecond SQLite read; instead the keypress renders the snippet and
/// this idle performs the single capped read, re-rendering full content. The
/// upgrade aborts when the selection moved before it runs, so a slow read can
/// never land on a stale selection.
fn schedule_preview_upgrade(state: &Rc<AppState>, entry: ClipboardEntry, generation: u64) {
    let state = Rc::clone(state);
    gtk::glib::idle_add_local_once(move || {
        let current = state.currently_previewed_entry_id.get();
        if current != Some(entry.id) || generation != state.preview_generation.get() {
            return;
        }
        let Some(full) = state
            .db
            .get_entry_preview(entry.id, MAX_FULL_PREVIEW_BYTES + 1)
            .ok()
            .flatten()
        else {
            return;
        };
        match &full.data {
            EntryData::Text | EntryData::Unknown => {
                let content = full
                    .text_content
                    .as_deref()
                    .or(full.preview_text.as_deref());
                if let Some(content) = content {
                    for channel in [&state.channels.text, &state.channels.code] {
                        if channel.parent().as_ref()
                            == Some(state.preview.upcast_ref::<gtk::Widget>())
                        {
                            state.preview.remove(channel);
                        }
                    }
                    if let Some(lang) = rsclip_core::syntax::detect_code_language(content) {
                        render_code_preview(&state, content, lang);
                    } else {
                        render_text_preview_state(&state, Some(content));
                    }
                }
            }
            _ => {}
        }
    });
}

fn render_text_or_code_preview(state: &Rc<AppState>, text: Option<&str>) {
    let raw = text.unwrap_or("");
    if let Some(lang) = rsclip_core::syntax::detect_code_language(raw) {
        state.channels.text_buffer.set_text("");
        render_code_preview(state, raw, lang);
    } else {
        state.channels.code_buffer.set_text("");
        render_text_preview_state(state, text);
    }
}

fn render_code_preview(state: &Rc<AppState>, text: &str, lang: rsclip_core::syntax::CodeLanguage) {
    rsclip_core::profiler::begin_phase("render_code_preview");
    let sanitized = sanitize_preview_text(text);

    let buffer = &state.channels.code_buffer;
    // Bound the highlight work: only payloads within the bound get the
    // context-engine parse; larger ones stay plain monospace, so their
    // language definition is never loaded at all.
    let highlight = sanitized.len() <= MAX_HIGHLIGHT_BYTES;
    // Freeze highlighting during the swap so the re-highlight of a newly set
    // language runs as one batch after the text is in place.
    buffer.set_highlight_syntax(false);
    if highlight && let Some(language) = crate::highlight::cached_language_for(lang) {
        buffer.set_language(Some(&language));
    }
    buffer.set_text(&sanitized);
    buffer.set_highlight_syntax(highlight);

    state.channels.code.vadjustment().set_value(0.0);
    state.channels.code.hadjustment().set_value(0.0);
    state.preview.append(&state.channels.code);
    rsclip_core::profiler::end_phase("render_code_preview");
}

fn render_text_preview_state(state: &Rc<AppState>, text: Option<&str>) {
    rsclip_core::profiler::begin_phase("render_text_preview");
    fill_text_preview(state, text);
    state.channels.text.vadjustment().set_value(0.0);
    state.preview.append(&state.channels.text);
    rsclip_core::profiler::end_phase("render_text_preview");
}

/// Fill the persistent plain-text buffer with the bounded preview payload.
fn fill_text_preview(state: &Rc<AppState>, text: Option<&str>) {
    let sanitized = sanitize_preview_text(text.unwrap_or(""));
    state.channels.text_buffer.set_text(&sanitized);
}

/// Cap preview text without splitting a UTF-8 code point, and detect binary payloads.
///
/// Bounded to 64 KiB so large text entries layout smoothly in `TextView` without
/// freezing the GTK main thread. Payloads containing interior null bytes are treated
/// as binary and presented with an explanatory notice instead of crashing GTK FFI.
fn bounded_full_preview(text: &str) -> std::borrow::Cow<'_, str> {
    if is_binary_payload(text) {
        return std::borrow::Cow::Borrowed(BINARY_PREVIEW_NOTICE);
    }

    if text.len() <= MAX_FULL_PREVIEW_BYTES {
        return std::borrow::Cow::Borrowed(text);
    }

    let mut end = MAX_FULL_PREVIEW_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut preview = String::with_capacity(end + FULL_PREVIEW_TRUNCATED_NOTICE.len());
    preview.push_str(&text[..end]);
    preview.push_str(FULL_PREVIEW_TRUNCATED_NOTICE);
    std::borrow::Cow::Owned(preview)
}

#[cfg(test)]
mod tests {
    use super::{
        BINARY_PREVIEW_NOTICE, FULL_PREVIEW_TRUNCATED_NOTICE, MAX_FULL_PREVIEW_BYTES,
        bounded_full_preview,
    };

    #[test]
    fn preview_is_bounded_on_utf8_boundary() {
        let text = "🦀".repeat(MAX_FULL_PREVIEW_BYTES);
        let preview = bounded_full_preview(&text);

        assert!(preview.ends_with(FULL_PREVIEW_TRUNCATED_NOTICE));
        assert!(preview.len() <= MAX_FULL_PREVIEW_BYTES + FULL_PREVIEW_TRUNCATED_NOTICE.len());
        assert!(std::str::from_utf8(preview.as_bytes()).is_ok());
    }

    #[test]
    fn small_preview_is_unchanged() {
        let text = "small clipboard entry";
        assert_eq!(bounded_full_preview(text), text);
    }

    #[test]
    fn binary_payload_shows_binary_notice() {
        let binary = "some\0binary\0data";
        assert_eq!(bounded_full_preview(binary), BINARY_PREVIEW_NOTICE);
    }
}
