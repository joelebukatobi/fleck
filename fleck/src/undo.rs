//! Per-note-window undo/redo history.
//!
//! A pure, widget-independent text history: the widget layer hands it full
//! body snapshots (`record`) and turns what `undo()`/`redo()` hand back into
//! an editor `Content` itself. Nothing here touches iced or libcosmic types,
//! so it is unit-testable without a running application.
//!
//! ## Coalescing rule
//!
//! Every `record` call names an [`EditKind`] (`Insert` or `Delete`) and
//! whether it must start a fresh undo group (`force_boundary`). A new group
//! starts whenever any of the following holds; otherwise the edit is folded
//! into the current group in place, so one undo reverts the whole group at
//! once instead of one keystroke at a time:
//!
//! - `force_boundary` is set. Callers pass this for anything that isn't an
//!   ordinary keystroke continuing the current word: a paste, pressing
//!   Enter, typing a whitespace character, or indent/unindent. The edit
//!   this is set on itself *starts* the new group - so typing a space
//!   closes out the word before it as its own undo step, and whatever is
//!   typed next coalesces into the group the space started.
//! - There is no group yet - the very first edit, or the edit right after an
//!   undo/redo (undo/redo always end the current group).
//! - More than [`COALESCE_PAUSE`] has passed since the group's last edit - a
//!   paused-then-resumed edit reads as a separate thought.
//! - The edit's [`EditKind`] differs from the group's, e.g. typing then
//!   deleting or the reverse - so undo doesn't have to replay a deletion
//!   before it can undo the insertion that preceded it.

use std::time::{Duration, Instant};

/// Longest gap between two edits that still belong to the same undo group.
const COALESCE_PAUSE: Duration = Duration::from_secs(1);

/// Number of undo groups retained before the oldest is dropped.
const DEFAULT_CAP: usize = 200;

/// Whether an edit added or removed text. Used only to decide whether it
/// continues the current undo group (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Insert,
    Delete,
}

/// A per-window undo/redo history of full-text snapshots.
///
/// Starts with one entry - the text the note had when its window opened -
/// so undo can never reach past that (see `new`).
pub struct UndoHistory {
    /// Committed group snapshots. `entries[index]` is the current text.
    /// Entries after `index` are the redo stack.
    entries: Vec<String>,
    index: usize,
    cap: usize,
    group_started_at: Option<Instant>,
    group_kind: Option<EditKind>,
}

impl UndoHistory {
    /// Starts a history with `initial` as the only, current entry. This is
    /// the floor undo can never go past - it must be the note's text at the
    /// moment its window opened.
    pub fn new(initial: impl Into<String>) -> Self {
        Self::with_cap(initial, DEFAULT_CAP)
    }

    /// Like `new`, with an explicit cap on retained groups (tests only need
    /// a small one to exercise the eviction).
    pub fn with_cap(initial: impl Into<String>, cap: usize) -> Self {
        Self {
            entries: vec![initial.into()],
            index: 0,
            cap: cap.max(1),
            group_started_at: None,
            group_kind: None,
        }
    }

    /// The current text.
    pub fn text(&self) -> &str {
        &self.entries[self.index]
    }

    /// Records the body's new full text after an edit. See the module docs
    /// for how `kind`/`force_boundary` decide whether this joins the
    /// current undo group or starts a new one. A new edit always discards
    /// any redo history above it, exactly as in every other editor.
    pub fn record(&mut self, new_text: impl Into<String>, kind: EditKind, force_boundary: bool) {
        let new_text = new_text.into();
        if new_text == self.text() {
            return;
        }
        let now = Instant::now();
        let boundary = force_boundary
            || self.group_started_at.is_none()
            || self.group_kind != Some(kind)
            || self.group_started_at.is_some_and(|started| now.duration_since(started) > COALESCE_PAUSE);

        // A new edit - whether it starts a group or joins one - always
        // invalidates whatever was undone before it.
        self.entries.truncate(self.index + 1);

        if boundary {
            self.entries.push(new_text);
            self.index += 1;
            if self.entries.len() > self.cap {
                self.entries.remove(0);
                self.index -= 1;
            }
        } else {
            self.entries[self.index] = new_text;
        }

        self.group_started_at = Some(now);
        self.group_kind = Some(kind);
    }

    /// Steps back one undo group, returning its text, or `None` if already
    /// at the oldest (window-open) state.
    pub fn undo(&mut self) -> Option<&str> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        self.group_started_at = None;
        self.group_kind = None;
        Some(self.text())
    }

    /// Steps forward one undo group, returning its text, or `None` if
    /// there's nothing to redo.
    pub fn redo(&mut self) -> Option<&str> {
        if self.index + 1 >= self.entries.len() {
            return None;
        }
        self.index += 1;
        self.group_started_at = None;
        self.group_kind = None;
        Some(self.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_returns_the_previous_text() {
        let mut h = UndoHistory::new("a");
        h.record("ab", EditKind::Insert, true);
        assert_eq!(h.undo(), Some("a"));
    }

    #[test]
    fn redo_after_undo_returns_it_again() {
        let mut h = UndoHistory::new("a");
        h.record("ab", EditKind::Insert, true);
        h.undo();
        assert_eq!(h.redo(), Some("ab"));
    }

    #[test]
    fn new_edit_after_undo_clears_redo() {
        let mut h = UndoHistory::new("a");
        h.record("ab", EditKind::Insert, true);
        h.undo();
        h.record("ac", EditKind::Insert, true);
        assert_eq!(h.redo(), None);
    }

    #[test]
    fn typing_coalesces_into_one_group() {
        let mut h = UndoHistory::new("");
        h.record("h", EditKind::Insert, false);
        h.record("he", EditKind::Insert, false);
        h.record("hel", EditKind::Insert, false);
        assert_eq!(h.undo(), Some(""), "the whole burst of typing must undo in one step");
    }

    #[test]
    fn a_group_boundary_splits_groups() {
        let mut h = UndoHistory::new("");
        h.record("hi", EditKind::Insert, false);
        // Typing a space forces a boundary (see module docs): it starts a
        // new group that "there" then coalesces into, keeping "hi" as its
        // own separate undo step.
        h.record("hi ", EditKind::Insert, true);
        h.record("hi there", EditKind::Insert, false);
        assert_eq!(h.undo(), Some("hi"));
        assert_eq!(h.undo(), Some(""));
    }

    #[test]
    fn deletion_after_insertion_starts_a_new_group() {
        let mut h = UndoHistory::new("");
        h.record("hi", EditKind::Insert, false);
        h.record("h", EditKind::Delete, false);
        assert_eq!(h.undo(), Some("hi"));
        assert_eq!(h.undo(), Some(""));
    }

    #[test]
    fn undo_on_empty_history_does_nothing() {
        let mut h = UndoHistory::new("only");
        assert_eq!(h.undo(), None);
        assert_eq!(h.text(), "only");
    }

    #[test]
    fn redo_with_nothing_undone_does_nothing() {
        let mut h = UndoHistory::new("only");
        assert_eq!(h.redo(), None);
    }

    #[test]
    fn cap_drops_the_oldest_entries() {
        let mut h = UndoHistory::with_cap("0", 3);
        h.record("1", EditKind::Insert, true);
        h.record("2", EditKind::Insert, true);
        h.record("3", EditKind::Insert, true);
        h.record("4", EditKind::Insert, true);

        assert_eq!(h.text(), "4");
        assert_eq!(h.undo(), Some("3"));
        assert_eq!(h.undo(), Some("2"));
        assert_eq!(h.undo(), None, "the oldest entries (\"0\", \"1\") must have been evicted");
    }
}
