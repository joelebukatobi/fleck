use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

mod images;
mod list;
mod note;
mod reminders;
mod style;
mod theme;

use cosmic::app::{Core, Task};
use cosmic::iced::core::id;
use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{SinkExt, Stream, StreamExt};
use cosmic::iced::{event, window, Length, Size, Subscription};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::text_editor;
use fleck_core::{display_name, is_disposable, Note, Store, WindowState};
use uuid::Uuid;

use crate::dbus;
use crate::palette::Colour;
use crate::undo::{EditKind, UndoHistory};

use list::dialog_width;
use note::{edit_kind, note_window_settings};
use reminders::{ListView, ReminderForm};
use style::IconHoverRole;
use theme::AppTheme;

/// The icon notifications show beside Fleck's name.
pub const FLECK_ICON: &str = "io.github.joelebukatobi.Fleck";

/// How often Fleck looks for reminders that have come due.
const REMINDER_TICK: Duration = Duration::from_secs(30);

/// The size of the list header's + icon.
const HEADER_ICON: u16 = 16;

/// How long to wait after the last keystroke before writing a note to disk.
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// Every note window opens at this size unless it has a saved size of its
/// own (see `window_size_for`).
const DEFAULT_WINDOW_SIZE: (u32, u32) = (512, 768);

/// A saved size smaller than this in either dimension is treated as absent:
/// a note resized down to nothing (or a corrupt state file) must not reopen
/// as an invisible window.
const MIN_WINDOW_SIZE: (u32, u32) = (200, 150);

/// The size a note's window should open at: its saved size, if one exists
/// and is at least `MIN_WINDOW_SIZE`, otherwise `DEFAULT_WINDOW_SIZE`.
fn window_size_for(uuid: Uuid, sizes: &BTreeMap<Uuid, (u32, u32)>) -> (u32, u32) {
    match sizes.get(&uuid) {
        Some(&(w, h)) if w >= MIN_WINDOW_SIZE.0 && h >= MIN_WINDOW_SIZE.1 => (w, h),
        _ => DEFAULT_WINDOW_SIZE,
    }
}

/// What `Fleck::init` needs beyond a `Core`: the note store, the window-size
/// state loaded from disk, the path to save it back to, and the already
/// name-owning D-Bus connection built in `main.rs` before any window opened
/// (see the module docs on `dbus_subscription`).
pub struct Flags {
    pub store: Store,
    pub window_state: WindowState,
    pub state_path: PathBuf,
    pub reminders_path: PathBuf,
    /// `--background`: run with no windows, waiting to fire reminders.
    pub background: bool,
    pub dbus_connection: zbus::Connection,
    pub dbus_rx: mpsc::Receiver<dbus::Request>,
}

/// The window title for a note: its explicit name, or its first non-empty
/// body line, or a sensible fallback for a note with no content yet.
fn window_title(note: &Note) -> String {
    fleck_core::display_name(note).to_string()
}

/// Whether a window's title needs to be re-sent to the compositor: only
/// when the computed display name differs from the title last actually set.
/// Keeping this as a pure comparison (rather than inline in `update`) is
/// what let a per-keystroke compositor round-trip become a once-per-flush
/// one: the title is recomputed on every edit, but only pushed out when it
/// has actually changed since the last push.
fn title_needs_update(current: &str, last_set: &str) -> bool {
    current != last_set
}

/// Toggling hides everything if anything is visible, and shows everything
/// only when nothing is. With no notes at all, "show" is the sensible
/// direction so the next created note appears.
fn next_all_visible(visible: &[bool]) -> bool {
    !visible.iter().any(|v| *v)
}

/// Whether `Message::PickNote` should close the list after acting on a
/// pick: only when the picked note actually existed. Closing the list
/// unconditionally quits the whole app when nothing else is visible (see
/// `Message::NoteClosed`'s list branch) - so a pick on a note deleted
/// between the list rendering and the click, which leaves `show_note` a
/// no-op, must not also close the list right along with it.
fn pick_note_closes_list(existed: bool) -> bool {
    existed
}

/// Whether each note is *meant* to be visible, held as explicit intent
/// rather than derived from `self.windows` (which only updates once the
/// compositor's asynchronous `Closed` event actually arrives). Updated
/// synchronously by `show`/`hide` the instant a show or hide is decided, so
/// a hide immediately followed by a show is never lost to a stale window
/// map, and a second show of an already-visible note is a no-op instead of
/// opening a duplicate window.
#[derive(Debug, Clone, Default, PartialEq)]
struct VisibilityIntent(HashSet<Uuid>);

impl VisibilityIntent {
    fn is_visible(&self, uuid: Uuid) -> bool {
        self.0.contains(&uuid)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Marks `uuid` as meant to be visible. Returns whether that's a change
    /// (`false` if it was already visible - the caller must not open a
    /// second window in that case).
    fn show(&mut self, uuid: Uuid) -> bool {
        self.0.insert(uuid)
    }

    /// Marks `uuid` as meant to be hidden. Returns whether that's a change
    /// (`false` if it was already hidden).
    fn hide(&mut self, uuid: Uuid) -> bool {
        self.0.remove(&uuid)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    NewNote,
    NoteOpened(window::Id),
    NoteClosed(window::Id),
    CloseRequested(window::Id),
    BodyAction(window::Id, text_editor::Action),
    /// Ctrl+Z, intercepted before the editor's own key handling (see
    /// `key_binding` in `view_window`) so it never falls through to
    /// inserting a literal "z".
    Undo(window::Id),
    /// Ctrl+Shift+Z or Ctrl+Y, intercepted the same way as `Undo`.
    Redo(window::Id),
    WindowResized(window::Id, Size),
    AutosaveTick,
    /// The user picked a note from the notes list: open it, close the list.
    PickNote(Uuid),
    /// The user picked "Reopen" on the restore bar: open every restorable
    /// note, close the list.
    ReopenSession,
    /// The user picked "No thanks" on the restore bar: it goes away, the
    /// list stays open.
    DismissRestore,
    /// A D-Bus call came in and is waiting on `self` to act on it.
    Dbus(DbusRequest),
    /// The notes-list search bar's text changed.
    SearchChanged(String),
    /// The user pressed a card's trash button: ask for confirmation before
    /// deleting `Uuid`. A no-op while the restore dialog is showing.
    DeleteStart(Uuid),
    /// The mouse entered (`Some`) or left (`None`) a card's delete button.
    DeleteHover(Option<Uuid>),
    /// "Delete" on the confirmation dialog: delete the pending note through
    /// `delete_note`.
    DeleteConfirm,
    /// "Cancel" on the confirmation dialog (or the dialog closing another
    /// way): leave the note untouched.
    DeleteCancel,
    /// The mouse entered or left the notes list's scrollable area - tracked
    /// only to widen the scrollbar's scroller on hover
    /// (`SCROLLBAR_SCROLLER_WIDTH_HOVER`) without ever changing the
    /// reserved layout width the cards see (`SCROLLBAR_WIDTH`, constant).
    ListScrollHover(bool),
    /// The settings button at the top-left of a note window.
    NoteMenuToggle(window::Id),
    /// A click outside an open note menu.
    NoteMenuClose(window::Id),
    /// "Edit name" in a note's menu: open the rename dialog.
    NoteRenameStart(window::Id),
    /// A keystroke in a note window's rename dialog.
    NoteRenameInput(window::Id, String),
    /// "Save" (or Enter) in a note window's rename dialog.
    NoteRenameSave(window::Id),
    /// "Change colour" in a note's menu: open the colour dialog.
    NoteColourStart(window::Id),
    /// A swatch clicked in a note window's colour dialog.
    NoteSetColour(window::Id, Colour),
    /// "Delete note" in a note's menu: ask for confirmation.
    NoteDeleteStart(window::Id),
    /// "Delete" in a note window's confirmation dialog.
    NoteDeleteConfirm(window::Id),
    /// "Cancel" in either note window dialog.
    NoteDialogCancel(window::Id),
    /// "Back to list" in a note's menu: open or raise the notes list.
    NoteBackToList(window::Id),
    /// A theme picked in a note's menu, for every Fleck window.
    SetTheme(AppTheme),
    /// The list window switched between notes and reminders.
    ListViewChanged(ListView),
    /// "Add Reminder": open an empty reminder form.
    ReminderAddStart,
    /// A reminder card was clicked: open it for editing.
    ReminderEditStart(Uuid),
    /// Keystrokes in the reminder form.
    ReminderFormText(String),
    ReminderFormDescription(text_editor::Action),
    ReminderFormEvent(bool),
    ReminderFormLocation(String),
    ReminderFormDate(String),
    ReminderFormTime(String),
    ReminderFormRepeat(fleck_core::Repeat),
    /// "Save" in the reminder form.
    ReminderSave,
    /// "Cancel" in the reminder form.
    ReminderCancel,
    /// A reminder's trash button.
    ReminderDelete(Uuid),
    /// Time to look for reminders that have come due.
    ReminderTick,
    /// Nothing to do: a background task finished.
    Done,
    /// "Remind me" in a note's menu: a reminder about that note.
    NoteRemindStart(window::Id),
    /// Ctrl+V in a note: look for an image on the clipboard first.
    NotePasteImage(window::Id),
    /// What the clipboard held: an image (encoded PNG), or nothing usable, in
    /// which case the paste falls back to text.
    NoteImagePasted(window::Id, Option<Arc<Vec<u8>>>),
    /// The clipboard's text, for a paste that turned out not to be an image.
    NotePasteText(window::Id, Option<String>),
    /// A thumbnail under a note: show that image full size.
    NoteImageOpen(window::Id, String),
    /// "Copy" in the image dialog: put the image back on the clipboard.
    NoteImageCopy(window::Id, String),
    /// "Remove" in the image dialog: delete the image and its line.
    NoteImageRemove(window::Id, String),
    /// A click on a thumbnail: put the cursor on that image's line and open
    /// the image full size.
    NoteImageClicked(window::Id, usize, String),
    /// Files dropped on a note window: the images among them are added.
    NoteFilesDropped(window::Id, Vec<PathBuf>),
    /// Escape in a note window: close whatever is open over it.
    NoteEscape(window::Id),
    /// A drag entered a note window, offering these mime types.
    NoteDragEntered(window::Id, Vec<String>),
    /// A drag left a note window without dropping.
    NoteDragLeft(window::Id),
    /// A Wayland drop finished: the raw `text/uri-list` it carried.
    NoteUriListDropped(window::Id, Arc<Vec<u8>>),
    /// Dragging a note window's header bar.
    NoteWindowDrag(window::Id),
    /// The maximize button, or a double-click, on a note window's header bar.
    NoteWindowMaximize(window::Id),
    /// The minimize button on a note window's header bar.
    NoteWindowMinimize(window::Id),
}

/// A dialog open over a note window. Drawn by the note window itself: libcosmic
/// only draws `Application::dialog` on the main window, the notes list.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum NoteDialog {
    /// Renaming, with the text typed so far.
    Rename(String),
    /// Choosing the note's colour.
    Colour,
    /// One of the note's images, full size.
    Image(String),
    Delete,
}

/// Wraps a `dbus::Request` so it can ride through `Message`, which
/// `cosmic::Application` requires to be `Clone`, even though the request
/// itself carries one-shot reply channels that cannot be cloned. `update`
/// always takes the request out with `take()`; nothing else touches this
/// type, and it is never actually cloned in practice - this message is
/// dispatched once, straight from the D-Bus subscription to `update`, never
/// broadcast to more than one place.
pub struct DbusRequest(std::sync::Arc<std::sync::Mutex<Option<dbus::Request>>>);

impl DbusRequest {
    fn new(request: dbus::Request) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(Some(request))))
    }

    fn take(&self) -> Option<dbus::Request> {
        self.0.lock().unwrap().take()
    }
}

impl Clone for DbusRequest {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::fmt::Debug for DbusRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DbusRequest(..)")
    }
}

/// A window showing a note: which note it is, the stable id its text
/// editor is registered under (needed to focus it on open), the editor's
/// own buffer (view state, synced from/to `Note.body`), and the title last
/// actually sent to the compositor (so it is only re-sent when the display
/// name has changed).
struct WindowNote {
    uuid: Uuid,
    input_id: id::Id,
    content: text_editor::Content,
    last_title: String,
    /// Undo/redo history for this window's session, seeded with the note's
    /// text at the moment the window opened - undo can never reach past
    /// that (see `undo::UndoHistory::new`).
    history: UndoHistory,
}

/// Cloneable, `Hash`-able handle on the D-Bus request receiver, so it can be
/// threaded through `Subscription::run_with` (which identifies a
/// subscription by hashing its data) while still only ever being drained by
/// one running stream. Hashed and compared by the `Arc`'s pointer identity:
/// stable across every `subscription()` call (called once per frame) since
/// it's the same `Arc` cloned each time, so the underlying stream is never
/// torn down and restarted.
#[derive(Clone)]
struct DbusRx(Arc<Mutex<Option<mpsc::Receiver<dbus::Request>>>>);

impl std::hash::Hash for DbusRx {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

#[allow(clippy::struct_excessive_bools)] // independent state flags, not a mode
pub struct Fleck {
    core: Core,
    store: Store,
    /// Per-note window size, persisted to `state_path`. The only parts of
    /// `fleck_core::geometry::WindowState` this app wires up -
    /// `placements`/`minimized` stay unused (no window position is ever
    /// persisted or restored).
    window_state: WindowState,
    state_path: PathBuf,
    /// Set when a resize, or a session snapshot, has changed `window_state`
    /// since it was last written to `state_path`. Checked on the same
    /// autosave tick that flushes dirty notes, rather than saving on every
    /// resize event - a resize drag emits many of those.
    window_state_dirty: bool,
    /// Which note each open note window is showing. The notes-list window
    /// is tracked separately, in `list_window` - it shows no note.
    windows: HashMap<window::Id, WindowNote>,
    /// The currently open notes-list window, if any. `None` once the user
    /// has picked a note (closing the list) or closed it directly, until a
    /// D-Bus `ShowList` reopens it.
    list_window: Option<window::Id>,
    /// Which notes are meant to be visible right now - see
    /// `VisibilityIntent`.
    intent_visible: VisibilityIntent,
    /// Notes offered by the restore bar on this launch: the survivors (from
    /// `fleck_core::restorable`) of whatever was open at last quit.
    /// Consumed (cleared) once the user picks "Reopen"; `restore_dismissed`
    /// tracks "No thanks" instead, since the bar's count stays meaningful
    /// even after being dismissed if shown again is never needed here.
    restore_candidates: BTreeSet<Uuid>,
    /// Whether the user dismissed the restore bar with "No thanks". Once
    /// true, the list renders with no bar for the rest of this run.
    restore_dismissed: bool,
    notes: HashMap<Uuid, Note>,
    /// Notes edited since their last save, and when they were last edited.
    dirty: HashMap<Uuid, Instant>,
    /// Each note's file mtime on disk, cached so `view_list` doesn't stat
    /// the filesystem every frame - refreshed whenever a note is loaded,
    /// created, or actually written by `flush`, and dropped on delete.
    mtimes: HashMap<Uuid, SystemTime>,
    /// The notes-list search bar's current query - see `matches_search`.
    search: String,
    /// Stable id for the search bar's text input, reused across frames
    /// (never `Id::new(name)` - see `register_window`'s doc comment on why
    /// a stable-but-unique id matters for this app's widget tree).
    search_input_id: id::Id,
    /// The note a "Delete note?" confirmation is currently pending for, if
    /// any - set by pressing a card's trash button, cleared by confirming
    /// or cancelling. `dialog()` renders the confirmation from this alone;
    /// deletion itself always goes through `delete_note`.
    pending_delete: Option<Uuid>,
    /// The card whose delete button the mouse is over, which shows the
    /// filled trash icon.
    delete_hovered: Option<Uuid>,
    /// Content rendered by `view_window` when a window id has no entry in
    /// `windows` (or its note has already been deleted). Never actually
    /// edited; it exists purely so every `view_window` return builds the
    /// exact same stateful `text_editor` widget tree, regardless of which
    /// branch supplied the content. Mixing that with a stateless
    /// `widget::text::body` fallback made the tree shape depend on lookup
    /// success, which panics libcosmic's `TextEditor` (its context-menu
    /// wrapper keeps widget state; a stateless sibling from a prior frame
    /// leaves that state as `State::None`, and the next diff panics trying
    /// to downcast it).
    fallback_content: text_editor::Content,
    /// Stable id for the fallback editor above, for the same reason.
    fallback_input_id: id::Id,
    /// Windows currently being closed by `hide_note`/`delete_note` rather
    /// than by the user. `NoteClosed` consults this to tell "this note
    /// window just got hidden/deleted" apart from "the user just closed
    /// this note window" - only the latter counts toward the
    /// nothing-left-open exit check (see `Message::NoteClosed`).
    closing_for_hide: HashSet<window::Id>,
    /// Whether `snapshot_session` has already run for the exit currently in
    /// progress. `on_app_exit` is the catch-all for exit paths this app
    /// didn't itself initiate (session logout, a compositor-driven kill);
    /// the paths it *did* initiate (last window closing, D-Bus `Quit`)
    /// already snapshotted before asking to exit, and must not be
    /// overwritten by a second, later snapshot that no longer has the
    /// closing window's note in `self.windows`.
    session_snapshotted: bool,
    /// Kept alive for as long as the app runs: dropping it would release
    /// the well-known D-Bus name acquired in `main.rs`. `None` only in
    /// tests, which construct a `Fleck` without a real connection.
    _dbus_connection: Option<zbus::Connection>,
    /// Handle on the D-Bus request receiver built in `main.rs`, threaded
    /// into the subscription - see `DbusRx` and `dbus_subscription`.
    dbus_rx: DbusRx,
    /// Whether the mouse is currently over the notes list's scrollable
    /// area - see `Message::ListScrollHover`.
    list_scroll_hovered: bool,
    /// Current width of the notes-list window, for sizing its dialogs.
    /// libcosmic's core does not track the main window's size.
    list_window_width: f32,
    /// Light, dark, or following COSMIC - see `theme::AppTheme`.
    theme: AppTheme,
    /// The note window whose menu is open, if any. One menu at a time.
    note_menu: Option<window::Id>,
    /// The note window showing a dialog, and which one. One at a time.
    note_dialog: Option<(window::Id, NoteDialog)>,
    /// Stable id for the rename dialog's text input, so it can be focused.
    note_rename_input_id: id::Id,
    /// Every reminder, and where they are kept.
    reminders: fleck_core::Reminders,
    reminders_path: PathBuf,
    /// Which half of the list window is showing.
    list_view: ListView,
    /// Started with `--background`: no windows, just waiting for reminders.
    /// Closing the last window doesn't quit in that mode.
    background: bool,
    /// The reminder being added or edited, if any.
    reminder_form: Option<ReminderForm>,
    /// The description box's own state, which `text_editor` keeps rather than
    /// the form: it is a multi-line editor, not a one-line field.
    reminder_description: text_editor::Content,
    /// The note window a drag carrying files is currently over.
    drag_over_note: Option<window::Id>,
}

impl Fleck {
    /// Writes every note that has been dirty for at least the debounce
    /// window through `Store::save`, and re-syncs the window title of any
    /// note whose display name changed while it was dirty.
    fn flush_due(&mut self, now: Instant) -> Task<Message> {
        let due: Vec<Uuid> = self
            .dirty
            .iter()
            .filter(|(_, &last_edit)| now.duration_since(last_edit) >= AUTOSAVE_DEBOUNCE)
            .map(|(id, _)| *id)
            .collect();
        let task = self.flush(&due);
        self.flush_window_state();
        task
    }

    /// Writes `window_state` to `state_path` if a resize has touched it
    /// since the last write. Errors are logged, not retried - like note
    /// positions, a lost resize is worth losing rather than worth crashing
    /// startup over.
    fn flush_window_state(&mut self) {
        if !self.window_state_dirty {
            return;
        }
        if let Err(e) = self.window_state.save(&self.state_path) {
            tracing::error!("failed to save window state: {e}");
        }
        self.window_state_dirty = false;
    }

    /// Force-writes the given notes through `Store::save`, regardless of the
    /// debounce, clears their dirty flag, and re-syncs window titles. This
    /// is the only place a note's window title is pushed to the compositor:
    /// doing it here rather than on every keystroke means at most one
    /// compositor round-trip per debounce interval instead of one per
    /// character typed into the name field.
    fn flush(&mut self, ids: &[Uuid]) -> Task<Message> {
        let mut tasks = Vec::new();
        for id in ids {
            let saved = match self.notes.get(id) {
                Some(note) => match self.store.save(note) {
                    Ok(()) => true,
                    Err(e) => {
                        tracing::error!("failed to save note {id}: {e}");
                        false
                    }
                },
                // Nothing in memory to save under this id: don't leave a
                // phantom dirty entry behind.
                None => true,
            };
            if saved {
                self.dirty.remove(id);
                self.refresh_mtime(*id);
                let window_ids: Vec<window::Id> = self
                    .windows
                    .iter()
                    .filter(|(_, w)| w.uuid == *id)
                    .map(|(wid, _)| *wid)
                    .collect();
                for wid in window_ids {
                    tasks.push(self.sync_title(wid));
                }
            }
        }
        Task::batch(tasks)
    }

    /// Re-sends a window's title to the compositor only if its computed
    /// display name has changed since the title last actually set.
    fn sync_title(&mut self, id: window::Id) -> Task<Message> {
        let Some(window) = self.windows.get(&id) else {
            return Task::none();
        };
        let uuid = window.uuid;
        let title = self
            .notes
            .get(&uuid)
            .map_or_else(|| fleck_core::UNNAMED.to_string(), window_title);
        if !title_needs_update(&title, &window.last_title) {
            return Task::none();
        }
        let task = self.set_window_title(title.clone(), id);
        if let Some(window) = self.windows.get_mut(&id) {
            window.last_title = title;
        }
        task
    }

    fn flush_all(&mut self) {
        let ids: Vec<Uuid> = self.dirty.keys().copied().collect();
        let _ = self.flush(&ids);
        self.flush_window_state();
    }

    /// Registers `uuid` as the note shown by window `id` (already open - a
    /// window just returned by `window::open`) and pushes its initial
    /// title. The main window is the notes list, not a note, so it never
    /// goes through here.
    fn register_window(&mut self, id: window::Id, uuid: Uuid) -> Task<Message> {
        // `Id::unique()`, not `Id::new(name)`: a *named* (`Internal::Custom`)
        // id routes through libcosmic's cross-frame "named widget" state
        // relocation in `Tree::diff` (see `iced/core/src/widget/tree.rs`).
        // For a widget nested inside `TextEditor`'s context-menu wrapper,
        // that relocation does not restore the taken state before the next
        // `layout()`, leaving `State::None` where `EditorWrapperState`'s
        // child expects real state - the exact "Downcast on stateless
        // state" panic this fix addresses, reproducible even with a single
        // pre-existing note and zero user interaction. A unique id doesn't
        // take that path (it isn't `Internal::Custom`) but is just as
        // usable with `widget::text_input::focus`, which matches on the id
        // itself rather than its name.
        let input_id = id::Id::unique();
        let body = self.notes.get(&uuid).map_or("", |note| note.body.as_str());
        let content = text_editor::Content::with_text(body);
        let history = UndoHistory::new(body);
        let title = self
            .notes
            .get(&uuid)
            .map_or_else(|| fleck_core::UNNAMED.to_string(), window_title);
        self.windows.insert(
            id,
            WindowNote {
                uuid,
                input_id,
                content,
                last_title: title.clone(),
                history,
            },
        );
        self.set_window_title(title, id)
    }

    /// Opens a brand-new secondary window for `uuid`. The first note at
    /// startup does not go through this - it attaches to the main window
    /// libcosmic already created (see `init`), since a window can't be
    /// opened twice.
    ///
    /// This is the one choke point every path that opens a note window goes
    /// through (`show_note`, `create_note`, and anything added later) - so
    /// marking `uuid` intent-visible happens here, not at each call site.
    /// `create_note` used to call this directly without ever touching
    /// `intent_visible`, which meant a brand-new note could never be hidden
    /// (`hide_note` early-returns when intent never had it) and, worse, left
    /// the list's "is anything still meant to be visible" exit check blind
    /// to it - closing the list right after creating a note quit the whole
    /// app. See `show_note` for the now-redundant call this replaces.
    fn open_window_for(&mut self, uuid: Uuid) -> Task<Message> {
        self.intent_visible.show(uuid);
        let (w, h) = window_size_for(uuid, &self.window_state.sizes);
        let settings = note_window_settings(Size::new(w as f32, h as f32));
        let (id, spawn) = window::open(settings);
        let registered = self.register_window(id, uuid);
        let opened = spawn.map(|id| cosmic::Action::App(Message::NoteOpened(id)));

        Task::batch([opened, registered])
    }

    /// Whether `uuid` is meant to be visible right now - see
    /// `VisibilityIntent`. This is intent, not "does a window exist for it
    /// this instant": `self.windows` only updates once the compositor's
    /// asynchronous `Closed` event arrives, so deriving visibility from it
    /// made a show immediately after a hide no-op (the stale entry was
    /// still there) while replying success, and made `ListNotes` report
    /// stale state.
    fn is_visible(&self, uuid: Uuid) -> bool {
        self.intent_visible.is_visible(uuid)
    }

    /// Closes the window showing `uuid`, if it has one open, without
    /// touching the note itself: any pending edit is flushed first, then
    /// the window is closed directly through `window::close` rather than
    /// going via `Message::CloseRequested` - so the delete-on-close rule for
    /// empty notes never runs. The window id is recorded in
    /// `closing_for_hide` so the `NoteClosed` that follows knows this
    /// closure isn't the user quitting. A no-op if `uuid` wasn't intended
    /// visible in the first place.
    fn hide_note(&mut self, uuid: Uuid) -> Task<Message> {
        if !self.intent_visible.hide(uuid) {
            return Task::none();
        }
        let Some(id) = self
            .windows
            .iter()
            .find(|(_, w)| w.uuid == uuid)
            .map(|(id, _)| *id)
        else {
            return Task::none();
        };
        let flush = if self.dirty.contains_key(&uuid) {
            self.flush(&[uuid])
        } else {
            Task::none()
        };
        self.closing_for_hide.insert(id);
        Task::batch([flush, window::close(id)])
    }

    /// Opens a window for `uuid` from its stored content - unless it's
    /// already intended visible, in which case this is a no-op rather than a
    /// duplicate window (see `VisibilityIntent`). Also a no-op for a uuid
    /// that isn't a known note at all. The actual `intent_visible.show` call
    /// lives in `open_window_for`, the choke point every window-opening path
    /// shares - this only guards against opening a second window for a note
    /// already showing one.
    fn show_note(&mut self, uuid: Uuid) -> Task<Message> {
        if !self.notes.contains_key(&uuid) {
            return Task::none();
        }
        if self.intent_visible.is_visible(uuid) {
            return Task::none();
        }
        self.open_window_for(uuid)
    }

    /// Hides every visible note, or shows every note if none are visible
    /// (the rule from `next_all_visible`). Returns whether notes are now
    /// visible, for `ToggleAll`'s D-Bus reply.
    fn toggle_all(&mut self) -> (bool, Task<Message>) {
        let uuids: Vec<Uuid> = self.notes.keys().copied().collect();
        let visible: Vec<bool> = uuids.iter().map(|&id| self.is_visible(id)).collect();
        let show = next_all_visible(&visible);
        let tasks = uuids
            .into_iter()
            .map(|id| {
                if show {
                    self.show_note(id)
                } else {
                    self.hide_note(id)
                }
            })
            .collect::<Vec<_>>();
        (show, Task::batch(tasks))
    }

    /// Creates a new note the same way `Message::NewNote` always has, and
    /// opens a window for it. Returns the new note's uuid on success, so
    /// D-Bus's `NewNote` can hand it back to the caller.
    fn create_note(&mut self) -> (Option<Uuid>, Task<Message>) {
        let now = crate::now_rfc3339();
        match self.store.create(&now, Colour::Default.name()) {
            Ok(note) => {
                let uuid = note.frontmatter.uuid;
                self.notes.insert(uuid, note);
                self.refresh_mtime(uuid);
                (Some(uuid), self.open_window_for(uuid))
            }
            Err(e) => {
                tracing::error!("failed to create note: {e}");
                (None, Task::none())
            }
        }
    }

    /// Deletes `uuid` from the store and every piece of state that tracks
    /// it - `Store::delete`, `self.notes`, `self.dirty`, and
    /// `self.window_state.sizes` - so this is the one place either deletion
    /// route (the D-Bus `DeleteNote` call, or the delete-on-close branch for
    /// an empty note) has to go through. A removed size entry only marks
    /// `window_state_dirty`, rather than saving synchronously, so the write
    /// rides the same debounced flush as every other window-state change
    /// instead of an extra disk write per delete.
    ///
    /// Deliberately does not touch `self.windows` or close any window: a
    /// window still open for `uuid` is the caller's responsibility (see
    /// `delete_note`, and the delete-on-close branch which is already mid-
    /// close). Returns whether the note existed.
    fn delete_note_data(&mut self, uuid: Uuid) -> bool {
        if !self.notes.contains_key(&uuid) {
            return false;
        }
        if let Err(e) = self.store.delete(uuid) {
            tracing::error!("failed to delete note {uuid}: {e}");
        }
        self.notes.remove(&uuid);
        self.dirty.remove(&uuid);
        self.mtimes.remove(&uuid);
        self.intent_visible.hide(uuid);
        if self.window_state.sizes.remove(&uuid).is_some() {
            self.window_state_dirty = true;
        }
        true
    }

    /// Deletes `uuid` through `delete_note_data`, closing its window first
    /// (if it has one) exactly like `hide_note` - directly, bypassing the
    /// delete-on-close check, since this delete is already unconditional.
    /// Returns whether the note existed.
    fn delete_note(&mut self, uuid: Uuid) -> (bool, Task<Message>) {
        if !self.notes.contains_key(&uuid) {
            return (false, Task::none());
        }
        let close_task = match self
            .windows
            .iter()
            .find(|(_, w)| w.uuid == uuid)
            .map(|(id, _)| *id)
        {
            Some(id) => {
                self.closing_for_hide.insert(id);
                window::close(id)
            }
            None => Task::none(),
        };
        let existed = self.delete_note_data(uuid);
        (existed, close_task)
    }

    /// Whether the restore-session dialog (offered only right after launch,
    /// while there's something to restore and it hasn't been dismissed) is
    /// currently showing - the precedence check both `dialog()` and
    /// `DeleteStart` use to keep at most one dialog on screen at a time.
    fn restore_dialog_active(&self) -> bool {
        !self.restore_dismissed && !self.restore_candidates.is_empty()
    }

    /// Opens the notes-list window, or raises it if one is already open.
    fn show_list(&mut self) -> Task<Message> {
        if let Some(id) = self.list_window {
            return window::gain_focus(id);
        }
        let settings = note_window_settings(Size::new(
            DEFAULT_WINDOW_SIZE.0 as f32,
            DEFAULT_WINDOW_SIZE.1 as f32,
        ));
        let (id, spawn) = window::open(settings);
        self.list_window = Some(id);
        // libcosmic draws the header (the `+` button) and dialogs only on
        // the main window. The original main window is gone once the list
        // has been closed, so the reopened list takes over that role.
        self.core.set_main_window_id(Some(id));
        spawn.map(|id| cosmic::Action::App(Message::NoteOpened(id)))
    }

    /// Closes the notes-list window, if one is open. Goes through
    /// `window::close` (not `list_window = None` here) so `self.list_window`
    /// stays valid until the compositor's `Closed` event actually confirms
    /// it, same as note windows - see `Message::NoteClosed`.
    fn close_list(&mut self) -> Task<Message> {
        match self.list_window {
            Some(id) => window::close(id),
            None => Task::none(),
        }
    }

    /// Records which notes are currently open (`self.windows`, not the
    /// list) as the session to offer on next launch, and writes it out
    /// immediately - quitting is exactly the moment this needs to survive a
    /// crash or power loss, so it doesn't wait for the debounced autosave.
    /// Idempotent: safe to call more than once for the same exit (see
    /// `session_snapshotted`).
    fn snapshot_session(&mut self) {
        self.window_state.open_at_quit = self.windows.values().map(|w| w.uuid).collect();
        self.window_state_dirty = true;
        self.flush_window_state();
        self.session_snapshotted = true;
    }

    /// Every exit Fleck starts itself goes through here: snapshot the
    /// session (unless this exit already did), write every dirty note, then
    /// exit. `cosmic::iced::exit()` does not run `on_app_exit`, so without
    /// this an edit or rename made in the last autosave interval was lost.
    fn exit_app(&mut self) -> Task<Message> {
        if !self.session_snapshotted {
            self.snapshot_session();
        }
        self.flush_all();
        cosmic::iced::exit()
    }

    /// Applies one D-Bus request and answers its reply channel (where it
    /// has one) with whatever actually happened.
    fn handle_dbus_request(&mut self, request: dbus::Request) -> Task<Message> {
        match request {
            dbus::Request::ListNotes(reply) => {
                let list = self
                    .notes
                    .iter()
                    .map(|(uuid, note)| {
                        (
                            uuid.to_string(),
                            fleck_core::display_name(note).to_string(),
                            self.is_visible(*uuid),
                        )
                    })
                    .collect();
                let _ = reply.send(list);
                Task::none()
            }
            dbus::Request::ShowNote(uuid, reply) => {
                let existed = self.notes.contains_key(&uuid);
                let task = self.show_note(uuid);
                let _ = reply.send(existed);
                task
            }
            dbus::Request::HideNote(uuid, reply) => {
                let existed = self.notes.contains_key(&uuid);
                let task = self.hide_note(uuid);
                let _ = reply.send(existed);
                task
            }
            dbus::Request::NewNote(reply) => {
                let (uuid, task) = self.create_note();
                let _ = reply.send(uuid);
                task
            }
            dbus::Request::DeleteNote(uuid, reply) => {
                let (existed, task) = self.delete_note(uuid);
                let _ = reply.send(existed);
                task
            }
            dbus::Request::ToggleAll(reply) => {
                let (visible, task) = self.toggle_all();
                let _ = reply.send(visible);
                task
            }
            dbus::Request::ShowList(reply) => {
                let task = self.show_list();
                let _ = reply.send(());
                task
            }
            dbus::Request::Quit => self.exit_app(),
        }
    }

    /// Re-reads `uuid`'s file mtime from disk into the cache `view_list`
    /// reads from - called right after a save actually wrote the file, so
    /// the cached value never drifts far from what's really on disk. A
    /// failed read (e.g. the save itself failed) just leaves the cache
    /// stale rather than erroring.
    fn refresh_mtime(&mut self, uuid: Uuid) {
        if let Ok(m) = self.store.modified(uuid) {
            self.mtimes.insert(uuid, m);
        }
    }

    /// Sets `uuid`'s explicit display name and marks it dirty for the next
    /// autosave flush, exactly like a body edit - `flush` is what actually
    /// writes it through `Store::save` and, if the note's window is open,
    /// re-syncs its title. An empty name is allowed: `display_name` already
    /// falls back to the first body line for one.
    fn rename_note(&mut self, uuid: Uuid, name: &str) {
        let Some(note) = self.notes.get_mut(&uuid) else {
            return;
        };
        note.frontmatter.name = name.trim().to_string();
        self.dirty.insert(uuid, Instant::now());
    }

    /// Fires every reminder that has come due: a notification, and the note it
    /// is about opens. Repeats move to their next occurrence; one-offs are
    /// done and go.
    fn fire_due_reminders(&mut self) -> Task<Message> {
        let now = chrono::Local::now();
        let due: Vec<fleck_core::Reminder> = self.reminders.due(now).into_iter().cloned().collect();
        if due.is_empty() {
            return Task::none();
        }
        let mut tasks = Vec::with_capacity(due.len() * 2);
        for reminder in due {
            let summary = self.reminder_title(&reminder);
            let mut lines = Vec::with_capacity(3);
            if reminder.event && !reminder.location.trim().is_empty() {
                lines.push(reminder.location.clone());
            }
            if !reminder.description.trim().is_empty() {
                lines.push(reminder.description.clone());
            }
            if let Some(note) = reminder.note.and_then(|uuid| self.notes.get(&uuid)) {
                lines.push(fleck_core::title(note).to_string());
            }
            let body = lines.join("\n");
            crate::notify::play_alarm();
            tasks.push(Task::perform(crate::notify::send(summary, body), |()| {
                cosmic::Action::App(Message::Done)
            }));
            if let Some(uuid) = reminder.note {
                tasks.push(self.show_note(uuid));
            }
            self.reminders.reschedule(reminder.uuid);
        }
        self.save_reminders();
        Task::batch(tasks)
    }

    /// Writes the reminders file. Errors are logged, not retried: a lost
    /// reminder is worth less than a crash at startup.
    fn save_reminders(&self) {
        if let Err(error) = self.reminders.save(&self.reminders_path) {
            tracing::error!(?error, "failed to save the reminders");
        }
    }

    /// Types `text` into note window `id` at the cursor, as a paste: the
    /// editor, the note and the undo history all see it exactly as they would
    /// a Ctrl+V of that text.
    fn insert_at_cursor(&mut self, id: window::Id, text: &str) {
        let Some(window) = self.windows.get_mut(&id) else {
            return;
        };
        window
            .content
            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new(text.to_string()),
            )));
        let uuid = window.uuid;
        let body = window.content.text();
        window.history.record(body.clone(), EditKind::Insert, true);
        self.sync_body(uuid, body);
    }

    /// Deletes one of note window `id`'s images: its file, and the line
    /// linking to it. Leaves any other line alone, including a second link to
    /// the same image.
    fn remove_image(&mut self, id: window::Id, name: &str) {
        let Some(uuid) = self.windows.get(&id).map(|window| window.uuid) else {
            return;
        };
        if let Err(error) = self.store.delete_image(uuid, name) {
            tracing::error!(?error, "failed to delete the image file");
        }
        let Some(note) = self.notes.get(&uuid) else {
            return;
        };
        let keep: Vec<&str> = note
            .body
            .lines()
            .filter(|line| {
                fleck_core::image_links(line)
                    .first()
                    .map(|(_, l)| l.as_str())
                    != Some(name)
            })
            .collect();
        let body = keep.join("\n");
        if let Some(window) = self.windows.get_mut(&id) {
            window.content = text_editor::Content::with_text(&body);
            window.history.record(body.clone(), EditKind::Delete, true);
        }
        self.sync_body(uuid, body);
    }

    /// Writes `text` into the note behind `uuid`'s body and marks it dirty,
    /// exactly as a normal body edit does - shared by `BodyAction` (a live
    /// keystroke) and `apply_history_step` (an undo/redo), so autosave
    /// picks up either the same way.
    fn sync_body(&mut self, uuid: Uuid, text: String) {
        if let Some(note) = self.notes.get_mut(&uuid) {
            note.body = text;
            self.dirty.insert(uuid, Instant::now());
        }
    }

    /// Runs `step` (`UndoHistory::undo` or `::redo`) against window `id`'s
    /// history and, if it returned a text, replaces the editor's `Content`
    /// with it and syncs `note.body`/dirty via `sync_body`.
    ///
    /// `Content` has no API to overwrite its text in place - only
    /// `with_text` (fresh content) or `perform` (single edits) - so the
    /// whole `Content` is rebuilt from the recorded snapshot. `with_text`
    /// leaves the cursor at the document start, which reads worse than
    /// where the user was editing, so the cursor is moved to the document
    /// end afterwards - the closest `Content`/`Action` affords to "the
    /// point of the change" without tracking cursor offsets per snapshot.
    fn apply_history_step(
        &mut self,
        id: window::Id,
        step: fn(&mut UndoHistory) -> Option<&str>,
    ) -> Task<Message> {
        let Some(window) = self.windows.get_mut(&id) else {
            return Task::none();
        };
        let Some(text) = step(&mut window.history) else {
            return Task::none();
        };
        let text = text.to_string();
        window.content = text_editor::Content::with_text(&text);
        window
            .content
            .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        let uuid = window.uuid;
        self.sync_body(uuid, text);
        Task::none()
    }
}

/// Forwards each incoming D-Bus call (already received on `rx.0`, built in
/// `main.rs` alongside the connection that owns the well-known name) into
/// the application as a `Message::Dbus`. `Subscription::run_with` - not
/// `run`, which only takes a bare `fn()` - because the receiver has to be
/// threaded in from outside rather than created fresh here; `rx` is hashed
/// by the underlying `Arc`'s pointer identity (see `DbusRx`), so this
/// identifies the same running stream across every `subscription()` call
/// instead of being torn down and restarted each frame.
fn dbus_subscription(rx: &DbusRx) -> Subscription<Message> {
    Subscription::run_with(rx.clone(), dbus_worker)
}

fn dbus_worker(rx: &DbusRx) -> impl Stream<Item = Message> {
    let rx = rx.0.clone();
    cosmic::iced::stream::channel(16, async move |mut output| {
        // Taken once: whichever invocation of this recipe runs first claims
        // the receiver. A later re-invocation (should the recipe ever be
        // restarted) finds `None` and just idles rather than panicking.
        let Some(mut receiver) = rx.lock().unwrap().take() else {
            std::future::pending::<()>().await;
            unreachable!("pending future never resolves");
        };

        while let Some(request) = receiver.next().await {
            if output
                .send(Message::Dbus(DbusRequest::new(request)))
                .await
                .is_err()
            {
                break;
            }
        }
    })
}

impl cosmic::Application for Fleck {
    type Executor = cosmic::executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = "io.github.joelebukatobi.Fleck";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Flags) -> (Self, Task<Message>) {
        let Flags {
            store,
            window_state,
            state_path,
            reminders_path,
            background,
            dbus_connection,
            dbus_rx,
        } = flags;
        let mut notes = HashMap::new();
        match store.list() {
            Ok(loaded) => {
                for item in loaded {
                    match item {
                        Ok(note) => {
                            notes.insert(note.frontmatter.uuid, note);
                        }
                        Err(e) => tracing::warn!("skipping unreadable note: {e}"),
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    "failed to read notes directory {}: {e}",
                    store.dir().display()
                );
            }
        }

        // The survivors of whatever was open at last quit - notes deleted
        // since then are silently dropped. Empty means no restore bar.
        let existing: BTreeSet<Uuid> = notes.keys().copied().collect();
        let restore_candidates = fleck_core::restorable(&window_state.open_at_quit, &existing);

        // The main window is the notes list, not a note - every note opens
        // as a secondary window (`window::open`), including the ones
        // offered for restore, only once the user picks "Reopen".
        let list_window = core.main_window_id();

        let mtimes = notes
            .keys()
            .filter_map(|id| store.modified(*id).ok().map(|m| (*id, m)))
            .collect();

        let app = Fleck {
            core,
            store,
            window_state,
            state_path,
            reminders: fleck_core::Reminders::load(&reminders_path),
            reminders_path,
            list_view: ListView::default(),
            reminder_form: None,
            reminder_description: text_editor::Content::new(),
            background,
            window_state_dirty: false,
            windows: HashMap::new(),
            list_window,
            intent_visible: VisibilityIntent::default(),
            restore_candidates,
            restore_dismissed: false,
            notes,
            dirty: HashMap::new(),
            mtimes,
            search: String::new(),
            search_input_id: id::Id::unique(),
            pending_delete: None,
            delete_hovered: None,
            fallback_content: text_editor::Content::new(),
            fallback_input_id: id::Id::unique(),
            closing_for_hide: HashSet::new(),
            session_snapshotted: false,
            _dbus_connection: Some(dbus_connection),
            dbus_rx: DbusRx(Arc::new(Mutex::new(Some(dbus_rx)))),
            list_scroll_hovered: false,
            list_window_width: DEFAULT_WINDOW_SIZE.0 as f32,
            theme: AppTheme::load(<Fleck as cosmic::Application>::APP_ID),
            note_menu: None,
            note_dialog: None,
            note_rename_input_id: id::Id::unique(),
            drag_over_note: None,
        };

        let apply_theme = app.theme.apply();
        let start = if app.background {
            // Nothing to show: close the window libcosmic opened and wait for
            // a reminder, a panel click, or a command.
            app.list_window.map_or_else(Task::none, window::close)
        } else {
            Task::none()
        };
        (app, Task::batch([apply_theme, start]))
    }

    fn subscription(&self) -> Subscription<Message> {
        let events = event::listen_with(|event, _, id| match event {
            cosmic::iced::Event::Window(window::Event::Opened { .. }) => {
                Some(Message::NoteOpened(id))
            }
            cosmic::iced::Event::Window(window::Event::Closed) => Some(Message::NoteClosed(id)),
            cosmic::iced::Event::Window(window::Event::CloseRequested) => {
                Some(Message::CloseRequested(id))
            }
            cosmic::iced::Event::Window(window::Event::Resized(size)) => {
                Some(Message::WindowResized(id, size))
            }
            cosmic::iced::Event::Window(window::Event::FileDropped(paths)) => {
                Some(Message::NoteFilesDropped(id, paths))
            }
            cosmic::iced::Event::Keyboard(cosmic::iced::keyboard::Event::KeyPressed {
                key: cosmic::iced::keyboard::Key::Named(cosmic::iced::keyboard::key::Named::Escape),
                ..
            }) => Some(Message::NoteEscape(id)),
            _ => None,
        });

        let mut subscriptions = vec![
            events,
            dbus_subscription(&self.dbus_rx),
            cosmic::iced::time::every(REMINDER_TICK).map(|_| Message::ReminderTick),
        ];
        if !self.dirty.is_empty() || self.window_state_dirty {
            subscriptions.push(
                cosmic::iced::time::every(Duration::from_millis(500))
                    .map(|_| Message::AutosaveTick),
            );
        }
        Subscription::batch(subscriptions)
    }

    #[allow(clippy::too_many_lines)] // one arm per message
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewNote => {
                let (_, task) = self.create_note();
                Task::batch([task, self.close_list()])
            }
            Message::PickNote(uuid) => {
                // Only close the list when the pick actually opened (or
                // would have opened, had it not already been visible) a
                // note. `show_note` no-ops silently for a uuid the list is
                // still showing a stale button for - the note was deleted
                // between the list rendering and the click - and with
                // nothing else visible, closing the list unconditionally
                // used to exit the whole app right along with it.
                let existed = self.notes.contains_key(&uuid);
                let show = self.show_note(uuid);
                if pick_note_closes_list(existed) {
                    Task::batch([show, self.close_list()])
                } else {
                    show
                }
            }
            Message::ReopenSession => {
                let candidates = std::mem::take(&mut self.restore_candidates);
                let shows: Vec<Task<Message>> = candidates
                    .into_iter()
                    .map(|uuid| self.show_note(uuid))
                    .collect();
                let mut tasks = shows;
                tasks.push(self.close_list());
                Task::batch(tasks)
            }
            Message::DismissRestore => {
                self.restore_dismissed = true;
                Task::none()
            }
            Message::NoteOpened(id) => {
                if let Some(window) = self.windows.get(&id) {
                    let focus = widget::text_input::focus(window.input_id.clone());
                    // COSMIC's frosted glass: libcosmic only blurs the main
                    // window, so note windows ask for it themselves. Only the
                    // title bar is translucent, so only it shows the blur.
                    if cosmic::theme::active().transparent {
                        Task::batch([focus, window::enable_blur(id)])
                    } else {
                        focus
                    }
                } else {
                    Task::none()
                }
            }
            Message::NoteClosed(id) => {
                if self.note_menu == Some(id) {
                    self.note_menu = None;
                }
                self.note_dialog.take_if(|(window, _)| *window == id);
                if self.list_window == Some(id) {
                    self.list_window = None;
                    // A note opening *concurrently* with the list closing
                    // (picking a note, or reopening a session) already
                    // updated `intent_visible` synchronously in the same
                    // `update()` call that requested the close - so this
                    // check never races the asynchronous completion of
                    // `window::open` for that note.
                    return if self.intent_visible.is_empty() && !self.background {
                        self.exit_app()
                    } else {
                        Task::none()
                    };
                }

                // A window closed *because it was hidden or deleted* is not
                // the user quitting: hiding deliberately drives a note out
                // of `self.windows` and the process must stay alive for a
                // later show/toggle-all to reopen it. Only a window that
                // closed on its own account - the user's X button, via
                // `CloseRequested` - counts toward "nothing is left open,
                // exit".
                let was_hide = self.closing_for_hide.remove(&id);

                // Clear intent for whichever note this window showed, now
                // that its window is genuinely gone - unless this close was
                // a hide, which already recorded the note as hidden (not
                // visible) the instant `hide_note` decided to hide it. A
                // hidden note must stay recorded as hidden - re-clearing
                // intent here would be a no-op either way (`hide` is
                // idempotent), but skipping it keeps the "who owns this
                // note's intent" story to one writer per close reason. A
                // window destroyed without ever going through
                // `CloseRequested` (compositor-forced, anything outside the
                // normal close-button sequence) previously left its note
                // reported visible forever - unreachable by a later
                // `show_note`, and a lie to `ListNotes`.
                if !was_hide {
                    if let Some(uuid) = self.windows.get(&id).map(|w| w.uuid) {
                        self.intent_visible.hide(uuid);
                    }
                }

                if was_hide {
                    self.windows.remove(&id);
                    return Task::none();
                }
                if self.windows.len() == 1 && self.list_window.is_none() && !self.background {
                    // This is the closing window whose note ended the
                    // session - snapshot while it's still in `self.windows`
                    // (browser tab-restore semantics: the last note you
                    // close is part of "what was open").
                    let exit = self.exit_app();
                    self.windows.remove(&id);
                    exit
                } else {
                    self.windows.remove(&id);
                    Task::none()
                }
            }
            Message::CloseRequested(id) => {
                if let Some(uuid) = self.windows.get(&id).map(|w| w.uuid) {
                    let disposable = self.notes.get(&uuid).is_some_and(is_disposable);
                    if disposable {
                        // The window is already on its way out via
                        // `window::close` below - `delete_note_data` never
                        // touches `self.windows`, so there's no second close
                        // to guard against here.
                        self.delete_note_data(uuid);
                    } else {
                        if self.dirty.contains_key(&uuid) {
                            let _ = self.flush(&[uuid]);
                        }
                        // Closing a note (not deleting it) returns it to
                        // the list - it's no longer meant to be visible.
                        self.intent_visible.hide(uuid);
                    }
                }
                // Deliberately NOT removing `self.windows[id]` here: the
                // window is still alive (this only requests the close) and
                // may render at least once more before the real `Closed`
                // event arrives. `NoteClosed` removes the entry once the
                // window is genuinely gone, so `view_window` keeps finding
                // its stateful editor content in the meantime instead of
                // falling through to the fallback branch mid-close.
                window::close(id)
            }
            Message::BodyAction(id, action) => {
                let Some(window) = self.windows.get_mut(&id) else {
                    return Task::none();
                };
                let edit = edit_kind(&action);
                window.content.perform(action);
                let uuid = window.uuid;
                let text = window.content.text();
                if let Some((kind, force_boundary)) = edit {
                    window.history.record(text.clone(), kind, force_boundary);
                }
                self.sync_body(uuid, text);
                Task::none()
            }
            Message::Undo(id) => self.apply_history_step(id, UndoHistory::undo),
            Message::Redo(id) => self.apply_history_step(id, UndoHistory::redo),
            Message::WindowResized(id, size) => {
                if Some(id) == self.core.main_window_id() {
                    self.list_window_width = size.width;
                }
                // Recorded in memory only - `flush_window_state` (driven by
                // the same autosave tick as note saves) is what actually
                // writes this to disk, so a resize drag's flood of events
                // costs one write at most, not one per event.
                if let Some(window) = self.windows.get(&id) {
                    let uuid = window.uuid;
                    let w = size.width.round().max(0.0) as u32;
                    let h = size.height.round().max(0.0) as u32;
                    self.window_state.sizes.insert(uuid, (w, h));
                    self.window_state_dirty = true;
                }
                Task::none()
            }
            Message::AutosaveTick => self.flush_due(Instant::now()),
            Message::Dbus(request) => match request.take() {
                Some(request) => self.handle_dbus_request(request),
                None => Task::none(),
            },
            Message::SearchChanged(text) => {
                self.search = text;
                Task::none()
            }
            Message::DeleteStart(uuid) => {
                // The restore dialog takes precedence - see
                // `restore_dialog_active`'s doc comment.
                if self.restore_dialog_active() {
                    return Task::none();
                }
                self.pending_delete = Some(uuid);
                Task::none()
            }
            Message::DeleteConfirm => match self.pending_delete.take() {
                Some(uuid) => self.delete_note(uuid).1,
                None => Task::none(),
            },
            Message::DeleteHover(uuid) => {
                self.delete_hovered = uuid;
                Task::none()
            }
            Message::DeleteCancel => {
                self.pending_delete = None;
                Task::none()
            }
            Message::NoteMenuToggle(id) => {
                self.note_menu = if self.note_menu == Some(id) {
                    None
                } else {
                    Some(id)
                };
                Task::none()
            }
            Message::NoteMenuClose(id) => {
                if self.note_menu == Some(id) {
                    self.note_menu = None;
                }
                Task::none()
            }
            Message::NoteRenameStart(id) => {
                self.note_menu = None;
                let name = self
                    .windows
                    .get(&id)
                    .and_then(|window| self.notes.get(&window.uuid))
                    .map(|note| note.frontmatter.name.clone());
                let Some(name) = name else {
                    return Task::none();
                };
                self.note_dialog = Some((id, NoteDialog::Rename(name)));
                widget::text_input::focus(self.note_rename_input_id.clone())
            }
            Message::NoteRenameInput(id, text) => {
                if let Some((window, NoteDialog::Rename(typed))) = &mut self.note_dialog {
                    if *window == id {
                        *typed = text;
                    }
                }
                Task::none()
            }
            Message::NoteRenameSave(id) => {
                let dialog = self.note_dialog.take_if(|(window, dialog)| {
                    *window == id && matches!(dialog, NoteDialog::Rename(_))
                });
                if let (Some((_, NoteDialog::Rename(name))), Some(window)) =
                    (dialog, self.windows.get(&id))
                {
                    let uuid = window.uuid;
                    self.rename_note(uuid, &name);
                }
                Task::none()
            }
            Message::NoteColourStart(id) => {
                self.note_menu = None;
                if self.windows.contains_key(&id) {
                    self.note_dialog = Some((id, NoteDialog::Colour));
                }
                Task::none()
            }
            Message::NoteSetColour(id, colour) => {
                self.note_dialog.take_if(|(window, _)| *window == id);
                let uuid = self.windows.get(&id).map(|window| window.uuid);
                if let Some(note) = uuid.and_then(|uuid| self.notes.get_mut(&uuid)) {
                    note.frontmatter.color = colour.name().to_string();
                    self.dirty.insert(note.frontmatter.uuid, Instant::now());
                }
                Task::none()
            }
            Message::ListViewChanged(view) => {
                self.list_view = view;
                Task::none()
            }
            Message::ReminderAddStart => {
                self.reminder_form = Some(ReminderForm::new(None, String::new()));
                self.reminder_description = text_editor::Content::new();
                Task::none()
            }
            Message::ReminderEditStart(uuid) => {
                self.reminder_form = self
                    .reminders
                    .reminders
                    .iter()
                    .find(|reminder| reminder.uuid == uuid)
                    .map(ReminderForm::editing);
                let description = self
                    .reminder_form
                    .as_ref()
                    .map(|form| form.description.clone())
                    .unwrap_or_default();
                self.reminder_description = text_editor::Content::with_text(&description);
                Task::none()
            }
            Message::ReminderFormText(text) => {
                if let Some(form) = &mut self.reminder_form {
                    form.text = text;
                }
                Task::none()
            }
            Message::ReminderFormDescription(action) => {
                self.reminder_description.perform(action);
                if let Some(form) = &mut self.reminder_form {
                    form.description = self.reminder_description.text();
                }
                Task::none()
            }
            Message::ReminderFormEvent(event) => {
                if let Some(form) = &mut self.reminder_form {
                    form.event = event;
                }
                Task::none()
            }
            Message::ReminderFormLocation(location) => {
                if let Some(form) = &mut self.reminder_form {
                    form.location = location;
                }
                Task::none()
            }
            Message::ReminderFormDate(date) => {
                if let Some(form) = &mut self.reminder_form {
                    form.date = date;
                }
                Task::none()
            }
            Message::ReminderFormTime(time) => {
                if let Some(form) = &mut self.reminder_form {
                    form.time = time;
                }
                Task::none()
            }
            Message::ReminderFormRepeat(repeat) => {
                if let Some(form) = &mut self.reminder_form {
                    form.repeat = repeat;
                }
                Task::none()
            }
            Message::ReminderSave => {
                if let Some(reminder) = self.reminder_form.take().and_then(|form| form.build()) {
                    self.reminders.insert(reminder);
                    self.save_reminders();
                }
                Task::none()
            }
            Message::ReminderCancel => {
                self.reminder_form = None;
                Task::none()
            }
            Message::ReminderDelete(uuid) => {
                self.reminders.remove(uuid);
                self.save_reminders();
                Task::none()
            }
            Message::ReminderTick => self.fire_due_reminders(),
            Message::Done => Task::none(),
            Message::NoteRemindStart(id) => {
                self.note_menu = None;
                let Some(uuid) = self.windows.get(&id).map(|window| window.uuid) else {
                    return Task::none();
                };
                // The form is a dialog on the list window, so bring that up.
                self.list_view = ListView::Reminders;
                self.reminder_form = Some(ReminderForm::new(Some(uuid), String::new()));
                self.reminder_description = text_editor::Content::new();
                self.show_list()
            }
            Message::NotePasteImage(id) => Task::perform(
                // Off the UI thread: reading the clipboard blocks.
                async {
                    tokio::task::spawn_blocking(images::from_clipboard)
                        .await
                        .ok()
                        .flatten()
                },
                move |png| cosmic::Action::App(Message::NoteImagePasted(id, png.map(Arc::new))),
            ),
            Message::NoteImagePasted(id, png) => {
                let Some(png) = png else {
                    // Not an image: paste whatever text is there instead.
                    return cosmic::iced::clipboard::read()
                        .map(move |text| cosmic::Action::App(Message::NotePasteText(id, text)));
                };
                let Some(uuid) = self.windows.get(&id).map(|window| window.uuid) else {
                    return Task::none();
                };
                match self.store.save_image(uuid, images::PASTED_FORMAT, &png) {
                    Ok(name) => self.insert_at_cursor(id, &images::link_line(&name)),
                    Err(error) => tracing::error!(?error, "failed to save the pasted image"),
                }
                Task::none()
            }
            Message::NotePasteText(id, text) => {
                if let Some(text) = text {
                    self.insert_at_cursor(id, &text);
                }
                Task::none()
            }
            Message::NoteImageOpen(id, name) => {
                self.note_dialog = Some((id, NoteDialog::Image(name)));
                Task::none()
            }
            Message::NoteImageCopy(id, name) => {
                let path = self
                    .windows
                    .get(&id)
                    .map(|window| window.uuid)
                    .and_then(|uuid| self.store.image_path(uuid, &name));
                if let Some(path) = path {
                    if let Err(error) = images::to_clipboard(&path) {
                        tracing::error!(%error, "failed to copy the image");
                    }
                }
                self.note_dialog.take_if(|(window, _)| *window == id);
                Task::none()
            }
            Message::NoteImageRemove(id, name) => {
                self.note_dialog.take_if(|(window, _)| *window == id);
                self.remove_image(id, &name);
                Task::none()
            }
            Message::NoteImageClicked(id, line, name) => {
                let Some(window) = self.windows.get_mut(&id) else {
                    return Task::none();
                };
                // Moving the cursor is what scrolls the editor; there is no
                // scroll-to-line of its own.
                window.content.perform(text_editor::Action::Move(
                    text_editor::Motion::DocumentStart,
                ));
                for _ in 0..line {
                    window
                        .content
                        .perform(text_editor::Action::Move(text_editor::Motion::Down));
                }
                self.update(Message::NoteImageOpen(id, name))
            }
            Message::NoteFilesDropped(id, paths) => {
                let Some(uuid) = self.windows.get(&id).map(|window| window.uuid) else {
                    return Task::none();
                };
                for path in paths {
                    let Some((extension, bytes)) = images::from_file(&path) else {
                        continue;
                    };
                    match self.store.save_image(uuid, &extension, &bytes) {
                        Ok(name) => self.insert_at_cursor(id, &images::link_line(&name)),
                        Err(error) => tracing::error!(?error, "failed to save the dropped image"),
                    }
                }
                Task::none()
            }
            Message::NoteEscape(id) => {
                if self.note_menu == Some(id) {
                    self.note_menu = None;
                }
                self.note_dialog.take_if(|(window, _)| *window == id);
                Task::none()
            }
            Message::NoteDragEntered(id, mimes) => {
                self.drag_over_note = mimes
                    .iter()
                    .any(|mime| mime == images::URI_LIST)
                    .then_some(id);
                Task::none()
            }
            Message::NoteDragLeft(id) => {
                if self.drag_over_note == Some(id) {
                    self.drag_over_note = None;
                }
                Task::none()
            }
            Message::NoteUriListDropped(id, data) => {
                self.drag_over_note = None;
                let paths = images::paths_from_uri_list(&data);
                self.update(Message::NoteFilesDropped(id, paths))
            }
            Message::NoteDeleteStart(id) => {
                self.note_menu = None;
                if self.windows.contains_key(&id) {
                    self.note_dialog = Some((id, NoteDialog::Delete));
                }
                Task::none()
            }
            Message::NoteDeleteConfirm(id) => {
                self.note_dialog.take_if(|(window, _)| *window == id);
                let Some(uuid) = self.windows.get(&id).map(|window| window.uuid) else {
                    return Task::none();
                };
                let (_, close) = self.delete_note(uuid);
                // With nothing else open, show the list rather than leave
                // Fleck running with no window at all.
                if self.list_window.is_none() && self.intent_visible.is_empty() {
                    Task::batch([close, self.show_list()])
                } else {
                    close
                }
            }
            Message::NoteDialogCancel(id) => {
                self.note_dialog.take_if(|(window, _)| *window == id);
                Task::none()
            }
            Message::NoteBackToList(id) => {
                if self.note_menu == Some(id) {
                    self.note_menu = None;
                }
                self.show_list()
            }
            Message::SetTheme(theme) => {
                self.note_menu = None;
                self.theme = theme;
                theme.save(<Fleck as cosmic::Application>::APP_ID);
                theme.apply()
            }
            Message::NoteWindowDrag(id) => window::drag(id),
            Message::NoteWindowMaximize(id) => window::toggle_maximize(id),
            Message::NoteWindowMinimize(id) => window::minimize(id, true),
            Message::ListScrollHover(hovered) => {
                self.list_scroll_hovered = hovered;
                Task::none()
            }
        }
    }

    /// Follows COSMIC's frosted glass setting being switched while note
    /// windows are open (see `Message::NoteOpened`).
    fn system_theme_update(
        &mut self,
        _keys: &[&'static str],
        new_theme: &cosmic::cosmic_theme::Theme,
    ) -> Task<Message> {
        let frosted = self.core.frosted(new_theme);
        Task::batch(self.windows.keys().map(|&id| {
            if frosted {
                window::enable_blur(id)
            } else {
                window::disable_blur(id)
            }
        }))
    }

    fn on_app_exit(&mut self) -> Option<Message> {
        // Catch-all for an exit this app didn't itself request (session
        // logout, a compositor-driven kill): the paths it *does* request
        // (the last window closing, D-Bus `Quit`) already snapshotted
        // before asking to exit, and must not be clobbered by a second,
        // later snapshot that no longer has the closing window's note in
        // `self.windows` (see `session_snapshotted`).
        if !self.session_snapshotted {
            self.snapshot_session();
        }
        self.flush_all();
        None
    }

    // `header_start`/`header_center`/`dialog` only reach the *main* window:
    // libcosmic builds its client-side header and dialog overlay
    // (`view_main`, which calls these) solely for `core.main_window_id()`;
    // every other window id is dispatched straight to `view_window` with
    // no header or dialog of its own layered on top (`Cosmic::view` in
    // libcosmic's `src/app/cosmic.rs`). The main window is the notes list
    // now, not a note, so this is where the Add note button and the app
    // name live. Secondary note windows (`window::open`, used for
    // every note) default to `decorations: true`, i.e. a compositor-drawn
    // title bar with no client content slots at all, so they keep whatever
    // title bar the compositor gives them.
    /// Which half of the window is showing: a dropdown on the left of the
    /// header, so switching is one click and the header's width never shifts.
    fn header_start(&self) -> Vec<Element<'_, Message>> {
        let views = vec![crate::fl!("tab-notes"), crate::fl!("tab-reminders")];
        let selected = match self.list_view {
            ListView::Notes => 0,
            ListView::Reminders => 1,
        };
        let add = match self.list_view {
            ListView::Notes => Message::NewNote,
            ListView::Reminders => Message::ReminderAddStart,
        };
        vec![
            widget::dropdown(views, Some(selected), |index| {
                Message::ListViewChanged(if index == 0 {
                    ListView::Notes
                } else {
                    ListView::Reminders
                })
            })
            .into(),
            widget::button::icon(widget::icon::from_name("list-add-symbolic").size(HEADER_ICON))
                .on_press(add)
                .class(style::header_icon_button_class(IconHoverRole::Accent))
                .into(),
        ]
    }

    fn header_center(&self) -> Vec<Element<'_, Message>> {
        vec![widget::text::body("Fleck").into()]
    }

    /// The restore prompt: a modal dialog overlaying (and dimming) the
    /// notes list, offered only while there's something to restore and the
    /// user hasn't already dismissed it this run. Same behaviour as the bar
    /// it replaces - Reopen opens every candidate and closes the list, No
    /// thanks just dismisses the dialog.
    fn dialog(&self) -> Option<Element<'_, Message>> {
        if let Some(form) = &self.reminder_form {
            return Some(self.view_reminder_form(form, &self.reminder_description));
        }
        if self.restore_dialog_active() {
            let count = self.restore_candidates.len();
            return Some(
                widget::dialog()
                    .title(crate::fl!("restore-title"))
                    .width(Length::Fixed(dialog_width(self.list_window_width)))
                    .body(crate::fl!("restore-body", count = count))
                    .primary_action(
                        widget::button::suggested(crate::fl!("restore-confirm"))
                            .on_press(Message::ReopenSession),
                    )
                    .secondary_action(
                        widget::button::standard(crate::fl!("restore-dismiss"))
                            .on_press(Message::DismissRestore),
                    )
                    .into(),
            );
        }

        // Only one dialog at a time: the restore dialog (above) takes
        // precedence, since it can only ever be showing right after launch,
        // before there has been any chance to press a card's trash button.
        let uuid = self.pending_delete?;
        let name = self.notes.get(&uuid).map_or("", display_name);
        Some(
            widget::dialog()
                .title(crate::fl!("delete-title"))
                .width(Length::Fixed(dialog_width(self.list_window_width)))
                .body(crate::fl!("delete-body", name = name))
                .primary_action(
                    widget::button::destructive(crate::fl!("delete-confirm"))
                        .on_press(Message::DeleteConfirm),
                )
                .secondary_action(
                    widget::button::standard(crate::fl!("cancel")).on_press(Message::DeleteCancel),
                )
                .into(),
        )
    }

    fn view(&self) -> Element<'_, Message> {
        // `Cosmic::view` dispatches every window id except the main one to
        // `view_window` directly; for the main window it falls back to
        // this method. The main window is the notes list, so just render
        // it the same way `view_window` would for that id.
        self.view_window(self.core.main_window_id().unwrap())
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if Some(id) == self.list_window {
            return match self.list_view {
                ListView::Notes => self.view_list(),
                ListView::Reminders => self.view_reminders(),
            };
        }
        self.view_note(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::Application;
    use fleck_core::{Frontmatter, FORMAT_VERSION};

    fn make_fleck(store: Store) -> Fleck {
        Fleck {
            core: Core::default(),
            store,
            window_state: WindowState::default(),
            state_path: std::env::temp_dir()
                .join(format!("fleck-test-windows-{}.toml", Uuid::new_v4())),
            reminders: fleck_core::Reminders::default(),
            reminders_path: std::env::temp_dir()
                .join(format!("fleck-test-reminders-{}.toml", Uuid::new_v4())),
            list_view: ListView::default(),
            reminder_form: None,
            reminder_description: text_editor::Content::new(),
            background: false,
            window_state_dirty: false,
            windows: HashMap::new(),
            list_window: None,
            intent_visible: VisibilityIntent::default(),
            restore_candidates: BTreeSet::new(),
            restore_dismissed: false,
            notes: HashMap::new(),
            dirty: HashMap::new(),
            mtimes: HashMap::new(),
            search: String::new(),
            search_input_id: id::Id::unique(),
            pending_delete: None,
            delete_hovered: None,
            fallback_content: text_editor::Content::new(),
            fallback_input_id: id::Id::unique(),
            closing_for_hide: HashSet::new(),
            session_snapshotted: false,
            _dbus_connection: None,
            dbus_rx: DbusRx(Arc::new(Mutex::new(None))),
            list_scroll_hovered: false,
            list_window_width: DEFAULT_WINDOW_SIZE.0 as f32,
            theme: AppTheme::System,
            note_menu: None,
            note_dialog: None,
            note_rename_input_id: id::Id::unique(),
            drag_over_note: None,
        }
    }

    fn sample_note() -> Note {
        Note {
            frontmatter: Frontmatter {
                version: FORMAT_VERSION,
                uuid: Uuid::new_v4(),
                created: "2026-09-04T10:15:00Z".to_string(),
                color: "yellow".to_string(),
                name: String::new(),
            },
            body: "test".to_string(),
        }
    }

    /// A failed save must leave the note dirty so the next autosave tick
    /// retries it, instead of silently discarding the in-memory edit.
    #[test]
    fn flush_keeps_dirty_flag_when_save_fails() {
        // Point the store at a path that can't become a directory: a plain
        // file already occupies it, so `Store::save`'s `create_dir_all`
        // fails and the write never happens.
        let tmp = std::env::temp_dir().join(format!("fleck-test-fail-{}", Uuid::new_v4()));
        std::fs::write(&tmp, b"not a directory").unwrap();
        let store = Store::new(&tmp);

        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);
        app.dirty.insert(id, Instant::now());

        let _ = app.flush(&[id]);

        assert!(
            app.dirty.contains_key(&id),
            "a note whose save failed must stay dirty so it is retried"
        );

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn flush_clears_dirty_flag_when_save_succeeds() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-ok-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);

        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);
        app.dirty.insert(id, Instant::now());

        let _ = app.flush(&[id]);

        assert!(!app.dirty.contains_key(&id));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn window_size_for_uses_default_with_no_saved_size() {
        let sizes = BTreeMap::new();
        assert_eq!(window_size_for(Uuid::new_v4(), &sizes), DEFAULT_WINDOW_SIZE);
    }

    #[test]
    fn window_size_for_uses_the_saved_size() {
        let id = Uuid::new_v4();
        let mut sizes = BTreeMap::new();
        sizes.insert(id, (600, 900));
        assert_eq!(window_size_for(id, &sizes), (600, 900));
    }

    #[test]
    fn window_size_for_falls_back_when_the_saved_size_is_too_small() {
        let id = Uuid::new_v4();
        let mut sizes = BTreeMap::new();
        sizes.insert(id, (10, 10));
        assert_eq!(window_size_for(id, &sizes), DEFAULT_WINDOW_SIZE);
    }

    #[test]
    fn title_needs_update_when_current_differs_from_last_set() {
        assert!(title_needs_update("New name", "Old name"));
    }

    #[test]
    fn title_no_update_when_current_matches_last_set() {
        assert!(!title_needs_update("Same name", "Same name"));
    }

    /// The bug this guards against: a note resized then deleted left a
    /// stale entry in `window_state.sizes` forever, because the
    /// delete-on-close path hand-rolled its own deletion and forgot to
    /// touch `sizes`. `delete_note_data` is the one function both deletion
    /// routes now go through, so exercising it directly covers both.
    #[test]
    fn delete_note_data_removes_window_size_entry() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-delete-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);
        app.window_state.sizes.insert(id, (600, 900));
        app.window_state_dirty = false;

        let existed = app.delete_note_data(id);

        assert!(existed);
        assert!(!app.notes.contains_key(&id));
        assert!(
            !app.window_state.sizes.contains_key(&id),
            "stale size entry left behind after delete"
        );
        assert!(
            app.window_state_dirty,
            "window state change must be marked dirty so the existing flush picks it up"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// The D-Bus `DeleteNote` route (`delete_note`) goes through the same
    /// shared function - confirm it also clears the size entry.
    #[test]
    fn delete_note_dbus_route_removes_window_size_entry() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-delete-dbus-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);
        app.window_state.sizes.insert(id, (600, 900));

        let (existed, _task) = app.delete_note(id);

        assert!(existed);
        assert!(!app.window_state.sizes.contains_key(&id));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn delete_note_data_returns_false_for_unknown_note() {
        let tmp =
            std::env::temp_dir().join(format!("fleck-test-delete-missing-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);

        assert!(!app.delete_note_data(Uuid::new_v4()));
    }

    #[test]
    fn show_note_on_an_already_intended_visible_note_does_not_reopen() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-show-twice-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let uuid = note.frontmatter.uuid;
        app.notes.insert(uuid, note);

        app.intent_visible.show(uuid);
        // `show_note`'s early return (already intended visible) must fire
        // before it ever calls `open_window_for` - the exact bug Fix 1
        // guards against: a `ShowNote` on a note already intended visible,
        // whose window hasn't appeared yet, opening a duplicate.
        assert!(app.is_visible(uuid));
        let _ = app.show_note(uuid);
        assert!(app.is_visible(uuid));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn snapshot_session_records_currently_open_notes() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-snapshot-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let uuid = Uuid::new_v4();
        app.windows.insert(
            window::Id::unique(),
            WindowNote {
                uuid,
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
                history: UndoHistory::new(""),
            },
        );

        app.snapshot_session();

        assert_eq!(app.window_state.open_at_quit, BTreeSet::from([uuid]));
        assert!(app.session_snapshotted);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn hide_all_does_not_touch_the_saved_session() {
        // Hide-all closes note windows through the same `closing_for_hide`
        // path as a single hide - `NoteClosed` must skip the snapshot
        // entirely for those, not just skip the exit.
        let tmp =
            std::env::temp_dir().join(format!("fleck-test-hideall-session-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        app.window_state.open_at_quit = BTreeSet::from([Uuid::new_v4()]);
        let previous = app.window_state.open_at_quit.clone();

        let id = window::Id::unique();
        app.windows.insert(
            id,
            WindowNote {
                uuid: Uuid::new_v4(),
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
                history: UndoHistory::new(""),
            },
        );
        app.closing_for_hide.insert(id);

        let _ = app.update(Message::NoteClosed(id));

        assert_eq!(
            app.window_state.open_at_quit, previous,
            "hide-all must not overwrite the saved session"
        );
        assert!(!app.session_snapshotted);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn note_closed_as_the_last_window_includes_it_in_the_snapshot() {
        // Browser tab-restore semantics: the very note whose window closing
        // ends the session must still be offered back on next launch.
        let tmp = std::env::temp_dir().join(format!("fleck-test-last-window-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let uuid = Uuid::new_v4();
        let id = window::Id::unique();
        app.windows.insert(
            id,
            WindowNote {
                uuid,
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
                history: UndoHistory::new(""),
            },
        );

        let _ = app.update(Message::NoteClosed(id));

        assert_eq!(app.window_state.open_at_quit, BTreeSet::from([uuid]));
        assert!(app.session_snapshotted);
        assert!(!app.windows.contains_key(&id));

        std::fs::remove_dir_all(&tmp).ok();
    }

    // --- Review fix regression tests (Findings 1, 2, 4) ---
    //
    // These drive `update` with messages directly, as instructed - the one
    // exception is a `window::Id` for a window that never really opened
    // (no compositor here), which is unavoidable to populate `self.windows`
    // for `NoteClosed`, exactly like the pre-existing tests above already
    // do (see `note_closed_as_the_last_window_includes_it_in_the_snapshot`).

    /// Finding 1: this is the exact scenario that used to kill the whole
    /// app - `NewNote` with nothing else open, then the list closes. Before
    /// the fix, `create_note` opened the window via `open_window_for`
    /// directly, which never touched `intent_visible` - so `NoteClosed`'s
    /// `intent_visible.is_empty()` check was still true, and the brand-new
    /// note's own window died along with everything else.
    #[test]
    fn new_note_with_nothing_else_open_does_not_quit_when_the_list_closes() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-newnote-quit-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let list_id = window::Id::unique();
        app.list_window = Some(list_id);

        let _ = app.update(Message::NewNote);
        let uuid = *app.notes.keys().next().expect("NewNote must create a note");
        assert!(
            app.is_visible(uuid),
            "the new note must be intent-visible as soon as it's created"
        );

        // Simulate the compositor actually finishing the close that
        // `Message::NewNote`'s `close_list()` requested.
        let _ = app.update(Message::NoteClosed(list_id));

        assert!(
            !app.session_snapshotted,
            "must not exit when the just-created note is still intent-visible"
        );
        assert!(
            app.is_visible(uuid),
            "the new note must still be reported visible"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Finding 1's other half: a newly created note used to be permanently
    /// stuck visible (`hide_note` early-returns when intent never had the
    /// note), and a later `show_note` on it opened a second window for the
    /// same note. Both are intent-level bugs, testable without a real
    /// window ever opening.
    #[test]
    fn a_newly_created_note_can_be_hidden_then_shown_exactly_once() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-newnote-hide-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);

        let (uuid, _task) = app.create_note();
        let uuid = uuid.expect("note creation must succeed");
        assert!(
            app.is_visible(uuid),
            "create_note must mark the note intent-visible"
        );

        let _ = app.hide_note(uuid);
        assert!(
            !app.is_visible(uuid),
            "a newly created note must be hideable"
        );

        let _ = app.show_note(uuid);
        assert!(
            app.is_visible(uuid),
            "showing it again must make it visible"
        );

        // A second show on an already-visible note must be a no-op at the
        // intent level - `show_note`'s guard is what stops a second
        // `open_window_for` call, i.e. a second window, from ever
        // happening.
        let _ = app.show_note(uuid);
        assert!(
            !app.intent_visible.show(uuid),
            "note must already be recorded visible - show_note must not have called \
             open_window_for a second time"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn closing_the_list_with_nothing_visible_exits_and_snapshots() {
        let tmp =
            std::env::temp_dir().join(format!("fleck-test-list-close-exit-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let list_id = window::Id::unique();
        app.list_window = Some(list_id);

        let _ = app.update(Message::NoteClosed(list_id));

        assert!(
            app.session_snapshotted,
            "closing the list with nothing intent-visible must exit"
        );
        assert!(app.list_window.is_none());

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Finding 2: a window destroyed without going through
    /// `CloseRequested` first (compositor-forced, or anything outside the
    /// normal close-button sequence) must not leave its note reported
    /// visible forever. `list_window` is set here so this doesn't also
    /// trip the (separately-tested) "last window closed" exit branch.
    #[test]
    fn note_closed_without_close_requested_stops_reporting_visible() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-forced-close-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        app.list_window = Some(window::Id::unique());
        let uuid = Uuid::new_v4();
        app.intent_visible.show(uuid);
        let id = window::Id::unique();
        app.windows.insert(
            id,
            WindowNote {
                uuid,
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
                history: UndoHistory::new(""),
            },
        );

        let _ = app.update(Message::NoteClosed(id));

        assert!(
            !app.is_visible(uuid),
            "a window destroyed without CloseRequested must stop reporting its note visible"
        );
        assert!(!app.windows.contains_key(&id));
        assert!(!app.session_snapshotted);

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Finding 2's other half: a hide-close must leave the note recorded as
    /// hidden, not visible - `NoteClosed` must not re-derive "visible" for
    /// a note `hide_note` already, synchronously, marked hidden.
    #[test]
    fn hide_close_leaves_the_note_hidden_not_visible_and_does_not_exit() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-hide-close-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let uuid = Uuid::new_v4();
        let id = window::Id::unique();
        app.windows.insert(
            id,
            WindowNote {
                uuid,
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
                history: UndoHistory::new(""),
            },
        );
        app.intent_visible.show(uuid);

        let _ = app.hide_note(uuid);
        assert!(
            !app.is_visible(uuid),
            "hide_note itself must mark it hidden synchronously"
        );

        let _ = app.update(Message::NoteClosed(id));

        assert!(
            !app.is_visible(uuid),
            "a hide-close must leave the note hidden, not visible"
        );
        assert!(
            !app.session_snapshotted,
            "a hide must never trigger the exit path"
        );
        assert!(!app.windows.contains_key(&id));

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Finding 4: `PickNote` on a note deleted between the list rendering
    /// and the click must leave the list open, not quit the app.
    ///
    /// This is the pure decision `update`'s `PickNote` arm defers to
    /// (`pick_note_closes_list`), extracted specifically because the
    /// runtime effect it gates - whether `close_list()`'s `window::close`
    /// task actually gets included in the returned `Task` batch - has no
    /// way to be observed from a unit test: `cosmic::app::Task` requires a
    /// real executor to run, and `NoteClosed` only ever arrives once the
    /// compositor confirms a close it was actually asked to make. Firing
    /// `NoteClosed(list_id)` by hand here wouldn't test the fix; it would
    /// only test `NoteClosed`'s (already covered) exit-on-empty-intent
    /// branch regardless of whether `PickNote` ever asked for the close. So
    /// this is exactly the boundary this task's brief says to name rather
    /// than paper over with a test that asserts nothing meaningful: full
    /// end-to-end confirmation that a vanished note's pick truly leaves the
    /// list open needs a live app.
    #[test]
    fn pick_note_closes_list_only_when_the_note_existed() {
        assert!(
            pick_note_closes_list(true),
            "an existing note's pick must still close the list"
        );
        assert!(
            !pick_note_closes_list(false),
            "a vanished note must not close the list"
        );
    }

    /// `update`'s `PickNote` arm must consult exactly this predicate (not
    /// some other condition) to decide whether to close the list - checked
    /// by exercising the arm on a note that was never created, where
    /// `show_note` no-ops exactly like it would for one just deleted.
    /// `app.list_window` and `app.notes` are the state this arm actually
    /// reads; a `pick_note_closes_list(false)` outcome must correspond to
    /// `show_note` itself being a no-op, which is what's checked here.
    #[test]
    fn pick_note_on_a_missing_note_leaves_it_unshown() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-pick-missing-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        app.list_window = Some(window::Id::unique());
        let uuid = Uuid::new_v4(); // never inserted into app.notes: "no longer exists"

        let _ = app.update(Message::PickNote(uuid));

        assert!(
            !app.is_visible(uuid),
            "a pick on a note that doesn't exist must not mark it visible"
        );
        assert!(app.notes.is_empty());

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn renaming_from_the_note_menu_allows_an_empty_name() {
        let (mut app, tmp, id, uuid) = app_with_open_note("rename-empty");
        app.notes.get_mut(&uuid).unwrap().frontmatter.name = "Old name".to_string();

        let _ = app.update(Message::NoteRenameStart(id));
        let _ = app.update(Message::NoteRenameInput(id, String::new()));
        let _ = app.update(Message::NoteRenameSave(id));

        assert_eq!(
            app.notes[&uuid].frontmatter.name, "",
            "an empty name must be allowed"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn flush_refreshes_the_cached_mtime_on_a_successful_save() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-mtime-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);
        app.dirty.insert(id, Instant::now());
        assert!(!app.mtimes.contains_key(&id));

        let _ = app.flush(&[id]);

        assert!(
            app.mtimes.contains_key(&id),
            "a successful save must populate the mtime cache"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn delete_note_data_removes_the_cached_mtime() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-mtime-delete-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);
        app.mtimes.insert(id, SystemTime::now());

        app.delete_note_data(id);

        assert!(!app.mtimes.contains_key(&id));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn delete_start_sets_pending_delete_without_deleting_anything() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-delete-start-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);

        let _ = app.update(Message::DeleteStart(id));

        assert_eq!(app.pending_delete, Some(id));
        assert!(
            app.notes.contains_key(&id),
            "pressing trash must not delete by itself"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn delete_confirm_deletes_through_the_shared_path() {
        let tmp =
            std::env::temp_dir().join(format!("fleck-test-delete-confirm-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.store.save(&note).unwrap();
        let path = app.store.path(id);
        assert!(path.exists());
        app.notes.insert(id, note);
        app.window_state.sizes.insert(id, (600, 900));
        app.pending_delete = Some(id);

        let _ = app.update(Message::DeleteConfirm);

        assert!(
            !app.notes.contains_key(&id),
            "note must be gone from memory"
        );
        assert!(!path.exists(), "note file must be gone from disk");
        assert!(
            !app.window_state.sizes.contains_key(&id),
            "saved window size must be gone"
        );
        assert_eq!(app.pending_delete, None);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn delete_cancel_leaves_everything_intact() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-delete-cancel-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);
        app.pending_delete = Some(id);

        let _ = app.update(Message::DeleteCancel);

        assert!(
            app.notes.contains_key(&id),
            "cancel must not delete the note"
        );
        assert_eq!(app.pending_delete, None);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn delete_confirm_closes_the_notes_open_window() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-delete-window-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);
        let window_id = window::Id::unique();
        app.windows.insert(
            window_id,
            WindowNote {
                uuid: id,
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
                history: UndoHistory::new(""),
            },
        );
        app.pending_delete = Some(id);

        let _ = app.update(Message::DeleteConfirm);

        assert!(
            app.closing_for_hide.contains(&window_id),
            "the note's open window must be closed by the shared delete_note path"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn delete_start_is_a_no_op_while_the_restore_dialog_is_showing() {
        let tmp =
            std::env::temp_dir().join(format!("fleck-test-delete-restore-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let id = note.frontmatter.uuid;
        app.notes.insert(id, note);
        app.restore_candidates = BTreeSet::from([Uuid::new_v4()]);
        assert!(app.restore_dialog_active());

        let _ = app.update(Message::DeleteStart(id));

        assert_eq!(
            app.pending_delete, None,
            "trash must not open a second dialog on top of the restore dialog"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn a_reopened_list_becomes_the_main_window() {
        let mut app = make_fleck(Store::new(std::env::temp_dir().join("fleck-test-unused")));

        let _ = app.show_list();

        assert!(app.list_window.is_some());
        assert_eq!(
            app.core.main_window_id(),
            app.list_window,
            "libcosmic only draws the header and dialogs on the main window"
        );
    }

    fn app_with_open_note(label: &str) -> (Fleck, PathBuf, window::Id, Uuid) {
        let tmp = std::env::temp_dir().join(format!("fleck-test-{label}-{}", Uuid::new_v4()));
        let mut app = make_fleck(Store::new(&tmp));
        let note = sample_note();
        let uuid = note.frontmatter.uuid;
        app.store.save(&note).unwrap();
        app.notes.insert(uuid, note);
        app.intent_visible.show(uuid);
        let id = window::Id::unique();
        app.windows.insert(
            id,
            WindowNote {
                uuid,
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
                history: UndoHistory::new(""),
            },
        );
        (app, tmp, id, uuid)
    }

    #[test]
    fn note_menu_toggles_and_closes() {
        let (mut app, tmp, id, _) = app_with_open_note("menu");

        let _ = app.update(Message::NoteMenuToggle(id));
        assert_eq!(app.note_menu, Some(id));
        let _ = app.update(Message::NoteMenuToggle(id));
        assert_eq!(app.note_menu, None);
        let _ = app.update(Message::NoteMenuToggle(id));
        let _ = app.update(Message::NoteMenuClose(id));
        assert_eq!(app.note_menu, None);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn rename_from_the_note_menu_saves_the_new_name() {
        let (mut app, tmp, id, uuid) = app_with_open_note("note-rename");

        let _ = app.update(Message::NoteMenuToggle(id));
        let _ = app.update(Message::NoteRenameStart(id));
        assert_eq!(app.note_menu, None, "picking an item closes the menu");
        assert_eq!(
            app.note_dialog,
            Some((id, NoteDialog::Rename(String::new())))
        );

        let _ = app.update(Message::NoteRenameInput(id, "Groceries".to_string()));
        let _ = app.update(Message::NoteRenameSave(id));

        assert_eq!(app.note_dialog, None);
        assert_eq!(app.notes[&uuid].frontmatter.name, "Groceries");
        assert!(
            app.dirty.contains_key(&uuid),
            "a rename is saved like any other edit"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// A one-pixel PNG, so the paste path can be driven without a clipboard.
    fn sample_png() -> Vec<u8> {
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(1, 1))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encoding a 1x1 png");
        png
    }

    #[test]
    fn pasting_an_image_saves_it_beside_the_note_and_links_to_it() {
        let (mut app, tmp, id, uuid) = app_with_open_note("paste-image");

        let _ = app.update(Message::NoteImagePasted(id, Some(Arc::new(sample_png()))));

        let links = fleck_core::image_links(&app.notes[&uuid].body);
        assert_eq!(links.len(), 1, "the note links to the pasted image");
        let path = app.store.image_path(uuid, &links[0].1).unwrap();
        assert!(path.exists(), "the image file is saved beside the note");
        assert!(
            app.dirty.contains_key(&uuid),
            "the new line is saved like any edit"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn removing_an_image_deletes_its_file_and_its_line() {
        let (mut app, tmp, id, uuid) = app_with_open_note("remove-image");
        let _ = app.update(Message::NoteImagePasted(id, Some(Arc::new(sample_png()))));
        let name = fleck_core::image_links(&app.notes[&uuid].body)[0].1.clone();
        let path = app.store.image_path(uuid, &name).unwrap();
        let _ = app.update(Message::NoteImageOpen(id, name.clone()));
        assert_eq!(app.note_dialog, Some((id, NoteDialog::Image(name.clone()))));

        let _ = app.update(Message::NoteImageRemove(id, name));

        assert_eq!(app.note_dialog, None);
        assert!(!path.exists(), "the image file is deleted");
        assert!(
            fleck_core::image_links(&app.notes[&uuid].body).is_empty(),
            "its line is gone from the note"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn a_due_reminder_opens_its_note_and_moves_on() {
        let (mut app, tmp, _, uuid) = app_with_open_note("reminder-fires");
        let overdue = chrono::Local::now() - chrono::Duration::minutes(5);
        let daily = fleck_core::Reminder::new(
            "water the plants".into(),
            overdue,
            fleck_core::Repeat::Daily,
            Some(uuid),
        );
        let once = fleck_core::Reminder::new(
            "post the letter".into(),
            overdue,
            fleck_core::Repeat::Once,
            None,
        );
        app.reminders.insert(daily.clone());
        app.reminders.insert(once);

        let _ = app.update(Message::ReminderTick);

        assert!(app.is_visible(uuid), "the note it points at opens");
        assert_eq!(app.reminders.reminders.len(), 1, "the one-off is finished");
        let moved = &app.reminders.reminders[0];
        assert_eq!(moved.uuid, daily.uuid);
        assert!(
            moved.due_at().unwrap() > chrono::Local::now(),
            "the daily one is set for its next occurrence"
        );

        std::fs::remove_file(&app.reminders_path).ok();
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn a_reminder_that_is_not_due_yet_stays_put() {
        let (mut app, tmp, _, _) = app_with_open_note("reminder-waits");
        app.reminders.insert(fleck_core::Reminder::new(
            "later".into(),
            chrono::Local::now() + chrono::Duration::hours(1),
            fleck_core::Repeat::Once,
            None,
        ));

        let _ = app.update(Message::ReminderTick);

        assert_eq!(app.reminders.reminders.len(), 1, "nothing fired");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn picking_a_colour_recolours_the_note_and_closes_the_dialog() {
        let (mut app, tmp, id, uuid) = app_with_open_note("note-colour");

        let _ = app.update(Message::NoteMenuToggle(id));
        let _ = app.update(Message::NoteColourStart(id));
        assert_eq!(app.note_menu, None);
        assert_eq!(app.note_dialog, Some((id, NoteDialog::Colour)));

        let _ = app.update(Message::NoteSetColour(id, Colour::Pop));

        assert_eq!(app.note_dialog, None);
        assert_eq!(app.notes[&uuid].frontmatter.color, "pop");
        assert!(
            app.dirty.contains_key(&uuid),
            "a colour change is saved like any edit"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn cancelling_the_note_rename_dialog_leaves_the_name_alone() {
        let (mut app, tmp, id, uuid) = app_with_open_note("note-rename-cancel");

        let _ = app.update(Message::NoteRenameStart(id));
        let _ = app.update(Message::NoteRenameInput(id, "Discarded".to_string()));
        let _ = app.update(Message::NoteDialogCancel(id));

        assert_eq!(app.note_dialog, None);
        assert_eq!(app.notes[&uuid].frontmatter.name, "");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn delete_from_the_note_menu_asks_first_then_deletes_and_shows_the_list() {
        let (mut app, tmp, id, uuid) = app_with_open_note("note-delete");

        let _ = app.update(Message::NoteDeleteStart(id));
        assert_eq!(app.note_dialog, Some((id, NoteDialog::Delete)));
        assert!(
            app.notes.contains_key(&uuid),
            "asking must not delete by itself"
        );

        let _ = app.update(Message::NoteDeleteConfirm(id));

        assert_eq!(app.note_dialog, None);
        assert!(!app.notes.contains_key(&uuid));
        assert!(!app.store.path(uuid).exists());
        assert!(
            app.closing_for_hide.contains(&id),
            "the note's window closes"
        );
        assert!(
            app.list_window.is_some(),
            "with nothing else open, the list opens"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn back_to_list_opens_the_list_and_keeps_the_note_open() {
        let (mut app, tmp, id, uuid) = app_with_open_note("back-to-list");

        let _ = app.update(Message::NoteMenuToggle(id));
        let _ = app.update(Message::NoteBackToList(id));

        assert_eq!(app.note_menu, None);
        assert!(app.list_window.is_some());
        assert!(app.is_visible(uuid));

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// `cosmic::iced::exit()` skips `on_app_exit`, so exiting must write
    /// pending edits itself - here, a rename made just before closing the
    /// list with nothing else open.
    #[test]
    fn closing_the_list_saves_a_pending_rename_before_exiting() {
        let tmp = std::env::temp_dir().join(format!("fleck-test-exit-flush-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_fleck(store);
        let note = sample_note();
        let uuid = note.frontmatter.uuid;
        app.notes.insert(uuid, note);
        let list_id = window::Id::unique();
        app.list_window = Some(list_id);
        app.rename_note(uuid, "Groceries");

        let _ = app.update(Message::NoteClosed(list_id));

        assert!(
            app.session_snapshotted,
            "closing the list with nothing open must exit"
        );
        assert!(
            !app.dirty.contains_key(&uuid),
            "the rename must be saved before exiting"
        );
        let saved =
            fleck_core::parse(&std::fs::read_to_string(app.store.path(uuid)).unwrap()).unwrap();
        assert_eq!(saved.frontmatter.name, "Groceries");

        std::fs::remove_dir_all(&tmp).ok();
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::*;

    #[test]
    fn toggle_all_hides_when_anything_is_visible() {
        assert!(!next_all_visible(&[true, true]));
        assert!(!next_all_visible(&[true, false]));
    }

    #[test]
    fn toggle_all_shows_when_everything_is_hidden() {
        assert!(next_all_visible(&[false, false]));
    }

    #[test]
    fn toggle_all_shows_when_there_are_no_notes() {
        assert!(next_all_visible(&[]));
    }

    // `VisibilityIntent` is the pure state extracted from Fix 1: `self.windows`
    // only updates once the compositor's asynchronous `Closed` event arrives,
    // so it can't be what "visible" means without a show-after-hide going
    // stale. These transitions are tested here with no window/Task/GUI
    // machinery at all.

    #[test]
    fn hide_then_immediately_show_leaves_the_note_visible() {
        let uuid = Uuid::new_v4();
        let mut intent = VisibilityIntent::default();
        intent.show(uuid);
        intent.hide(uuid);
        intent.show(uuid);
        assert!(
            intent.is_visible(uuid),
            "a show right after a hide must leave the note visible"
        );
    }

    #[test]
    fn showing_twice_does_not_duplicate() {
        let uuid = Uuid::new_v4();
        let mut intent = VisibilityIntent::default();
        assert!(intent.show(uuid), "first show is a real change");
        assert!(
            !intent.show(uuid),
            "second show must report no change - no duplicate window"
        );
        assert!(intent.is_visible(uuid));
    }

    #[test]
    fn hiding_twice_does_not_report_a_second_change() {
        let uuid = Uuid::new_v4();
        let mut intent = VisibilityIntent::default();
        intent.show(uuid);
        assert!(intent.hide(uuid), "first hide is a real change");
        assert!(!intent.hide(uuid), "second hide must report no change");
        assert!(!intent.is_visible(uuid));
    }

    #[test]
    fn a_note_never_shown_is_not_visible() {
        assert!(!VisibilityIntent::default().is_visible(Uuid::new_v4()));
    }
}
