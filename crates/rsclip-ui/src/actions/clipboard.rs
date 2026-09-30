use std::rc::Rc;

use anyhow::{Context, Result};
use gtk4::prelude::*;
use rsclip_core::Database;
use rsclip_core::models::SecretEntry;
use rsclip_core::paste::{copy_entry, write_clipboard};

use crate::events::queue_write;
use crate::state::{
    AfterCopy, AppState, CopyRequest, CopyResponse, WriteOp, advance_clipboard_serial,
};

/// Copy an entry on the copy worker; [`finish_entry_copy`] runs when it is done.
pub(crate) fn queue_entry_copy(state: &Rc<AppState>, id: i64, then: AfterCopy) -> Result<()> {
    let serial = advance_clipboard_serial(state);
    state
        .copy_request_tx
        .send(CopyRequest { id, then, serial })
        .map_err(|_| anyhow::anyhow!("copy worker stopped"))
}

/// Copy one entry to the clipboard. Runs on the copy worker.
pub(crate) fn copy_entry_by_id(db: &Database, id: i64) -> Result<()> {
    let entry = db.get_entry(id)?.context("entry no longer exists")?;
    copy_entry(&entry)
}

/// Apply a finished worker copy on the GTK thread.
pub(crate) fn finish_entry_copy(state: &Rc<AppState>, copy: CopyResponse) {
    if copy.result.is_ok()
        && let Err(err) = queue_write(state, WriteOp::TouchEntry(copy.id))
    {
        crate::actions::set_footer(state, &format!("Update failed: {err:#}"));
    }
    // A later clipboard write or closing the overlay superseded this request:
    // its paste would insert another value or land in a session the user
    // already left.
    if copy.serial != state.clipboard_serial.get() {
        return;
    }
    match (copy.result, copy.then) {
        (Err(err), AfterCopy::Paste) => {
            crate::actions::set_footer(state, &format!("Paste failed: {err}"))
        }
        (Err(err), AfterCopy::Report) => {
            crate::actions::set_footer(state, &format!("Copy failed: {err}"))
        }
        (Ok(()), AfterCopy::Report) => crate::actions::set_footer(state, "Copied selected entry"),
        (Ok(()), AfterCopy::Paste) => {
            let window = state
                .list
                .root()
                .and_then(|root| root.downcast::<gtk4::ApplicationWindow>().ok());
            if let Some(window) = window {
                crate::window::close_overlay_and_paste(state, &window);
            }
        }
    }
}

pub(crate) fn copy_secret(state: &Rc<AppState>, secret: &SecretEntry) -> Result<()> {
    copy_text(state, &secret.value)?;
    queue_write(state, WriteOp::TouchSecret(secret.id))
}

pub(crate) fn copy_text(state: &AppState, text: &str) -> Result<()> {
    advance_clipboard_serial(state);
    write_clipboard("text/plain", text.as_bytes())
}
