use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;

use crate::actions::refresh::ensure_row_rendered;
use crate::actions::set_footer;
use crate::components::preview::{render_preview, render_secret_preview};
use crate::state::{
    AppState, AppView, current_entry, current_entry_index, current_secret, current_secret_index,
    row_index_for_entry, row_index_for_secret,
};

// Key-repeat can enqueue preview renders faster than GTK can lay them out; a
// rebuild of the preview pane costs tens of milliseconds (TextBuffer + TextView
// layout), so per-keypress synchronous rebuilds stutter. `PENDING_PREVIEW`
// coalesces them: each keypress schedules a single latest-wins render, and the
// glib main loop executes at most one per iteration.
thread_local! {
    static PENDING_PREVIEW: Cell<bool> = const { Cell::new(false) };
}

fn schedule_preview_render<R>(render: impl FnOnce() -> R + 'static) {
    if PENDING_PREVIEW.with(|flag| flag.get()) {
        return;
    }
    PENDING_PREVIEW.with(|flag| flag.set(true));
    gtk::glib::idle_add_local_once(move || {
        PENDING_PREVIEW.with(|flag| flag.set(false));
        render();
    });
}

pub(crate) fn next_selection_index(
    current: Option<usize>,
    delta: i32,
    count: usize,
) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match current {
        Some(index) => {
            let next = (index as i32 + delta).clamp(0, count as i32 - 1) as usize;
            (next != index).then_some(next)
        }
        None => {
            if delta >= 0 {
                Some(0)
            } else {
                Some(count.saturating_sub(1))
            }
        }
    }
}

pub(crate) fn move_selection(state: &Rc<AppState>, delta: i32) {
    let count = match *state.view.borrow() {
        AppView::Clipboard => state.entries_total.get(),
        AppView::Secrets => state.secrets_total.get(),
    };

    let current = match *state.view.borrow() {
        AppView::Clipboard => current_entry_index(state),
        AppView::Secrets => current_secret_index(state),
    };

    let Some(next) = next_selection_index(current, delta, count) else {
        return;
    };

    if let Err(err) = ensure_row_rendered(state, next) {
        set_footer(state, &format!("Selection failed: {err:#}"));
        return;
    }

    let row_index = match *state.view.borrow() {
        AppView::Clipboard => row_index_for_entry(state, next),
        AppView::Secrets => row_index_for_secret(state, next),
    };
    if let Some(row) = row_index.and_then(|index| state.list.row_at_index(index)) {
        state.list.select_row(Some(&row));
        scroll_row_into_view(state, &row);
    }
}

// Render the preview for the currently selected row, coalesced per main-loop
// iteration so key-repeat never queues a backlog of synchronous rebuilds.
pub(crate) fn schedule_preview_for_current_selection(state: &Rc<AppState>) {
    let state = Rc::clone(state);
    schedule_preview_render(move || {
        if state.virtual_list_update.get() {
            return;
        }
        match *state.view.borrow() {
            AppView::Clipboard => {
                if let Some(entry) = current_entry(&state) {
                    render_preview(&state, &entry);
                }
            }
            AppView::Secrets => {
                if let Some(secret) = current_secret(&state) {
                    render_secret_preview(&state, &secret);
                }
            }
        }
    });
}

pub(crate) fn mark_selected_row(list: &gtk::ListBox, selected: Option<&gtk::ListBoxRow>) {
    let mut child = list.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        widget.remove_css_class("selected-entry");
    }

    if let Some(row) = selected {
        row.add_css_class("selected-entry");
    }
}

pub(crate) fn scroll_row_into_view(state: &Rc<AppState>, row: &gtk::ListBoxRow) {
    let Some(bounds) = row.compute_bounds(&state.list) else {
        return;
    };

    let adjustment = &state.list_adjustment;
    let viewport_top = adjustment.value();
    let viewport_bottom = viewport_top + adjustment.page_size();
    let row_top = f64::from(bounds.y());
    let row_bottom = row_top + f64::from(bounds.height());

    let target = if row_top < viewport_top {
        row_top
    } else if row_bottom > viewport_bottom {
        row_bottom - adjustment.page_size()
    } else {
        return;
    };

    let max_value = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
    let clamped_target = target.clamp(adjustment.lower(), max_value);
    if (clamped_target - adjustment.value()).abs() >= 1.0 {
        adjustment.set_value(clamped_target);
    }
}

#[cfg(test)]
mod tests {
    use super::next_selection_index;

    #[test]
    fn down_arrow_at_last_row_does_not_move() {
        assert_eq!(next_selection_index(Some(9), 1, 10), None);
        assert_eq!(next_selection_index(Some(0), 1, 1), None);
    }

    #[test]
    fn up_arrow_at_first_row_does_not_move() {
        assert_eq!(next_selection_index(Some(0), -1, 10), None);
        assert_eq!(next_selection_index(Some(0), -1, 1), None);
    }

    #[test]
    fn moving_within_bounds_succeeds() {
        assert_eq!(next_selection_index(Some(5), 1, 10), Some(6));
        assert_eq!(next_selection_index(Some(5), -1, 10), Some(4));
    }

    #[test]
    fn no_selection_initializes_to_first_or_last() {
        assert_eq!(next_selection_index(None, 1, 10), Some(0));
        assert_eq!(next_selection_index(None, -1, 10), Some(9));
    }

    #[test]
    fn empty_count_returns_none() {
        assert_eq!(next_selection_index(None, 1, 0), None);
        assert_eq!(next_selection_index(Some(0), 1, 0), None);
    }
}
