use std::rc::Rc;

use anyhow::Result;
use rsclip_core::models::{ClipboardEntry, SecretEntry};
use rsclip_core::paste::{copy_entry, write_clipboard};

use crate::events::queue_write;
use crate::state::{AppState, WriteOp};

pub(crate) fn copy_selected_entry(state: &Rc<AppState>, entry: &ClipboardEntry) -> Result<()> {
    copy_entry(entry)?;
    queue_write(state, WriteOp::TouchEntry(entry.id))
}

pub(crate) fn copy_secret(state: &Rc<AppState>, secret: &SecretEntry) -> Result<()> {
    write_clipboard("text/plain", secret.value.as_bytes())?;
    queue_write(state, WriteOp::TouchSecret(secret.id))
}

pub(crate) fn copy_text(text: &str) -> Result<()> {
    write_clipboard("text/plain", text.as_bytes())
}
