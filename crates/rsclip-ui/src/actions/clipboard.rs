use std::rc::Rc;

use anyhow::{Context, Result};
use gtk4::prelude::*;
use rsclip_core::Database;
use rsclip_core::models::SecretEntry;
use rsclip_core::paste::{copy_entry, write_clipboard};

use crate::events::{queue_write, send_to_worker};
use crate::state::{AfterCopy, AppState, CopyResponse, WorkerRequest, WriteOp};

/// Copy an entry on the list worker; [`finish_entry_copy`] runs when it is done.
pub(crate) fn queue_entry_copy(state: &Rc<AppState>, id: i64, then: AfterCopy) -> Result<()> {
    send_to_worker(state, WorkerRequest::CopyEntry { id, then })
}

/// Copy one entry to the clipboard. Runs on the list worker.
pub(crate) fn copy_entry_by_id(db: &Database, id: i64) -> Result<()> {
    let entry = db.get_entry(id)?.context("entry no longer exists")?;
    copy_entry(&entry)
}

/// Apply a finished worker copy on the GTK thread.
pub(crate) fn finish_entry_copy(state: &Rc<AppState>, copy: CopyResponse) {
    match (copy.result, copy.then) {
        (Err(err), AfterCopy::Paste) => {
            crate::actions::set_footer(state, &format!("Paste failed: {err}"))
        }
        (Err(err), AfterCopy::Report) => {
            crate::actions::set_footer(state, &format!("Copy failed: {err}"))
        }
        (Ok(()), AfterCopy::Report) => crate::actions::set_footer(state, "Copied selected entry"),
        (Ok(()), AfterCopy::Paste) => {
            // The user may have closed the overlay while the copy ran; the
            // clipboard is set either way, but only paste into a live request.
            let window = state
                .list
                .root()
                .and_then(|root| root.downcast::<gtk4::ApplicationWindow>().ok());
            if let Some(window) = window.filter(|window| window.is_visible()) {
                crate::window::close_overlay_and_paste(state, &window);
            }
        }
    }
}

pub(crate) fn copy_secret(state: &Rc<AppState>, secret: &SecretEntry) -> Result<()> {
    write_clipboard("text/plain", secret.value.as_bytes())?;
    queue_write(state, WriteOp::TouchSecret(secret.id))
}

pub(crate) fn copy_text(text: &str) -> Result<()> {
    write_clipboard("text/plain", text.as_bytes())
}
