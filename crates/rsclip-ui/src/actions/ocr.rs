use std::rc::Rc;

use anyhow::{Context, Result};
use gtk4::prelude::*;
use rsclip_core::EntryData;
use rsclip_core::ocr::run_tesseract_with_options;

use crate::actions::set_footer;
use crate::components::preview::render_preview;
use crate::state::AppState;

/// Start OCR for an image entry without blocking the GTK thread.
///
/// Tesseract takes seconds; running it inline froze the whole overlay (the
/// "Running OCR..." footer could not even paint). It now runs on a GIO worker
/// thread and the result is saved and rendered when it completes.
pub(crate) fn run_ocr_for_entry(state: &Rc<AppState>, entry_id: i64) -> Result<()> {
    if !state.ocr_enabled.get() {
        anyhow::bail!("OCR is disabled in config");
    }

    let entry = state
        .db
        .get_entry(entry_id)?
        .with_context(|| format!("entry {entry_id} not found"))?;
    let image_path = match &entry.data {
        EntryData::Image { file_path, .. } => file_path.clone(),
        _ => anyhow::bail!("entry is not an image"),
    };
    let language = state.ocr_language.borrow().clone();
    let command = state.ocr_command.borrow().clone();
    let timeout_seconds = state.ocr_timeout_seconds.get();

    set_footer(state, "Running OCR...");
    state.ocr_button.set_sensitive(false);

    let state = Rc::clone(state);
    gtk4::glib::spawn_future_local(async move {
        let ocr_language = language.clone();
        let result = gio::spawn_blocking(move || {
            run_tesseract_with_options(&image_path, &ocr_language, &command, timeout_seconds)
        })
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("OCR worker panicked")))
        .and_then(|text| finish_ocr(&state, entry_id, &language, &text));

        match result {
            Ok(()) => set_footer(&state, "OCR complete"),
            Err(err) => {
                set_footer(&state, &format!("OCR failed: {err:#}"));
                if state.currently_previewed_entry_id.get() == Some(entry_id) {
                    state.ocr_button.set_sensitive(true);
                }
            }
        }
    });
    Ok(())
}

fn finish_ocr(state: &Rc<AppState>, entry_id: i64, language: &str, text: &str) -> Result<()> {
    state.db.save_ocr_result(entry_id, language, text)?;
    let updated = state
        .db
        .get_entry(entry_id)?
        .with_context(|| format!("entry {entry_id} not found after OCR"))?;
    if let Some(slot) = state
        .entries
        .borrow_mut()
        .iter_mut()
        .find(|entry| entry.id == entry_id)
    {
        *slot = updated.clone();
    }
    // The user may have moved on while tesseract ran; only redraw if not.
    if state.currently_previewed_entry_id.get() == Some(entry_id) {
        crate::state::invalidate_preview_cache(state);
        render_preview(state, &updated);
    }
    Ok(())
}
