use std::rc::Rc;

use anyhow::{Context, Result};
use gtk::prelude::*;
use gtk4 as gtk;
use rsclip_core::secrets::{default_secret_alias, secret_value_from_entry};

use crate::actions::refresh::{
    current_selected_index, refresh_entries, refresh_entries_at_index,
    refresh_entries_preserving_selection,
};
use crate::actions::{set_footer, update_mode_controls};
use crate::dialogs::secret_alias::prompt_secret_alias;
use crate::events::queue_write;
use crate::state::{AppState, AppView, WriteOp, current_entry, current_full_entry, current_secret};

pub(crate) fn save_current_as_secret_dialog(state: &Rc<AppState>, parent: &gtk::Window) {
    let Some(entry) = current_full_entry(state) else {
        set_footer(state, "No selected entry to save");
        return;
    };
    let Some(value) = secret_value_from_entry(&entry) else {
        set_footer(state, "Only text-like entries can be saved as secrets");
        return;
    };
    let default_alias = default_secret_alias(&entry);

    prompt_secret_alias(
        state,
        parent,
        "Save Secret",
        &default_alias,
        move |state, alias| {
            queue_write(
                state,
                WriteOp::SaveSecret {
                    entry_id: entry.id,
                    alias,
                    value: value.clone(),
                },
            )?;
            *state.view.borrow_mut() = AppView::Secrets;
            *state.query.borrow_mut() = String::new();
            state.search_entry.set_text("");
            let placeholder = crate::window::search_placeholder(state.as_ref(), AppView::Secrets);
            state.search_entry.set_placeholder_text(Some(&placeholder));
            update_mode_controls(state);
            refresh_entries(state)?;
            set_footer(state, "Saved secret");
            Ok(())
        },
    );
}

pub(crate) fn rename_current_secret_dialog(state: &Rc<AppState>, parent: &gtk::Window) {
    let Some(secret) = current_secret(state) else {
        set_footer(state, "No selected secret to rename");
        return;
    };

    prompt_secret_alias(
        state,
        parent,
        "Rename Secret",
        &secret.alias,
        move |state, alias| {
            queue_write(
                state,
                WriteOp::RenameSecret {
                    id: secret.id,
                    alias,
                },
            )?;
            refresh_entries_preserving_selection(state)?;
            set_footer(state, "Renamed secret");
            Ok(())
        },
    );
}

pub(crate) fn toggle_pin(state: &Rc<AppState>) -> Result<()> {
    let entry = current_entry(state).context("no selected entry")?;
    queue_write(
        state,
        WriteOp::SetPinned {
            id: entry.id,
            pinned: !entry.pinned,
        },
    )?;
    refresh_entries_preserving_selection(state)
}

pub(crate) fn delete_current(state: &Rc<AppState>) -> Result<()> {
    let view = *state.view.borrow();
    let prev_index = current_selected_index(state);
    match view {
        AppView::Clipboard => {
            let entry = current_entry(state).context("no selected entry")?;
            queue_write(state, WriteOp::DeleteEntry(entry.id))?;
            refresh_entries_at_index(state, prev_index)?;
        }
        AppView::Secrets => {
            let secret = current_secret(state).context("no selected secret")?;
            let restore_clipboard = secret.source_entry_id.is_some();
            queue_write(state, WriteOp::DeleteSecret(secret.id))?;
            if restore_clipboard {
                *state.view.borrow_mut() = AppView::Clipboard;
                *state.query.borrow_mut() = String::new();
                state.search_entry.set_text("");
                let placeholder =
                    crate::window::search_placeholder(state.as_ref(), AppView::Clipboard);
                state.search_entry.set_placeholder_text(Some(&placeholder));
                update_mode_controls(state);
                refresh_entries(state)?;
            } else {
                refresh_entries_at_index(state, prev_index)?;
            }
        }
    }
    Ok(())
}
