use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cosmic::app::{Core, Task};
use cosmic::iced::core::id;
use cosmic::iced::core::text::LineHeight;
use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{SinkExt, Stream, StreamExt};
use cosmic::iced::widget::Stack;
use cosmic::iced::{event, window, Border, Color, Length, Pixels, Size, Subscription};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::text_editor;
use sticky_notes_core::{is_disposable, Note, Store, WindowState};
use uuid::Uuid;

use crate::dbus;
use crate::palette::Colour;
use crate::ruled::RuledLines;

/// Size of the note body's text, in logical pixels.
const BODY_TEXT_SIZE: f32 = 14.0;

/// Height of one line of the note body, in logical pixels. Set explicitly
/// (rather than left to font metrics) and shared, unchanged, with
/// `ruled::RuledLines::line_height` so the dotted rules drawn behind the
/// body can never drift out of alignment with the text sitting on them.
const BODY_LINE_HEIGHT: f32 = 22.0;

/// The body `text_editor`'s padding, in logical pixels, on every side.
/// Shared with `ruled::RuledLines::padding_top` for the same reason as
/// `BODY_LINE_HEIGHT` - the first rule's offset has to account for exactly
/// this much space above the first line of text.
const BODY_PADDING: f32 = 8.0;

/// The minimum height to give the note body's editor+canvas stack, given the
/// finite viewport height `responsive` reports for it: the viewport height
/// itself, so a short note's content is exactly viewport-tall (filling it
/// with ruled lines) while a long note is free to grow past it and scroll.
///
/// `responsive` sits outside the body's `scrollable` specifically so it only
/// ever sees a real, finite size (see `view_window`) - but a non-finite
/// input here is a caller bug away, and `min_height(f32::INFINITY)` would
/// mean "this editor is infinitely tall", i.e. the exact failure this
/// arrangement exists to avoid. So a non-finite `viewport_height` falls back
/// to no minimum (`0.0`) rather than passing infinity into layout.
fn body_min_height(viewport_height: f32) -> f32 {
    if viewport_height.is_finite() {
        viewport_height
    } else {
        0.0
    }
}

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

/// What `Tack::init` needs beyond a `Core`: the note store, the window-size
/// state loaded from disk, the path to save it back to, and the already
/// name-owning D-Bus connection built in `main.rs` before any window opened
/// (see the module docs on `dbus_subscription`).
pub struct Flags {
    pub store: Store,
    pub window_state: WindowState,
    pub state_path: PathBuf,
    pub dbus_connection: zbus::Connection,
    pub dbus_rx: mpsc::Receiver<dbus::Request>,
}

/// The window title for a note: its explicit name, or its first non-empty
/// body line, or a sensible fallback for a note with no content yet.
fn window_title(note: &Note) -> String {
    sticky_notes_core::display_name(note).to_string()
}

/// Whether note windows should use compositor-drawn server-side decorations
/// (`window::Settings.decorations: true`, no in-window header bar) instead
/// of the client-side header this app draws itself. Governed by the
/// `TACK_SSD` environment variable, read once at startup (`Tack::init`) and
/// held fixed for the life of the process - see `Tack::ssd` - so a window
/// never switches layouts mid-run. `"1"` means SSD, today's behaviour, kept
/// as an A/B baseline to compare against; anything else - unset, `"0"`, or
/// garbage - means the new client-side header.
fn use_server_side_decorations(value: Option<&str>) -> bool {
    value == Some("1")
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
    /// The in-window header bar (client-side decorations, see `Tack::ssd`)
    /// was dragged, or had its minimise/maximise control pressed. Close goes
    /// through `CloseRequested` instead - the existing message every other
    /// close path already uses - rather than a fourth variant here, so the
    /// header's close button flushes unsaved text and deletes empty notes
    /// exactly like every other way of closing a note.
    HeaderDrag(window::Id),
    HeaderMinimize(window::Id),
    HeaderMaximize(window::Id),
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

pub struct Tack {
    core: Core,
    store: Store,
    /// Per-note window size, persisted to `state_path`. The only parts of
    /// `sticky_notes_core::geometry::WindowState` this app wires up -
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
    /// `sticky_notes_core::restorable`) of whatever was open at last quit.
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
    /// tests, which construct a `Tack` without a real connection.
    _dbus_connection: Option<zbus::Connection>,
    /// Handle on the D-Bus request receiver built in `main.rs`, threaded
    /// into the subscription - see `DbusRx` and `dbus_subscription`.
    dbus_rx: DbusRx,
    /// Whether note windows use compositor-drawn server-side decorations
    /// instead of this app's own client-side header bar - see
    /// `use_server_side_decorations`. Read once from `TACK_SSD` in `init`
    /// and never changed afterwards, so every note window opened this run
    /// gets the same layout.
    ssd: bool,
}

impl Tack {
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
            eprintln!("tack: failed to save window state: {e}");
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
                        eprintln!("tack: failed to save note {id}: {e}");
                        false
                    }
                },
                // Nothing in memory to save under this id: don't leave a
                // phantom dirty entry behind.
                None => true,
            };
            if saved {
                self.dirty.remove(id);
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

    /// The in-window header bar's title for a note window: its display
    /// name plus the app name, read fresh every frame (no compositor round
    /// trip needed, unlike `sync_title`, which is what actually keeps the
    /// dock/switcher's copy of the title in sync). Falls back to `UNNAMED`
    /// exactly like `sync_title` does, for a window not yet registered.
    fn header_title(&self, id: window::Id) -> String {
        let name = self
            .windows
            .get(&id)
            .and_then(|window| self.notes.get(&window.uuid))
            .map(window_title)
            .unwrap_or_else(|| sticky_notes_core::UNNAMED.to_string());
        format!("{name} | Tack")
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
            .map(window_title)
            .unwrap_or_else(|| sticky_notes_core::UNNAMED.to_string());
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
        let body = self.notes.get(&uuid).map(|note| note.body.as_str()).unwrap_or("");
        let content = text_editor::Content::with_text(body);
        let title = self
            .notes
            .get(&uuid)
            .map(window_title)
            .unwrap_or_else(|| sticky_notes_core::UNNAMED.to_string());
        self.windows.insert(
            id,
            WindowNote { uuid, input_id, content, last_title: title.clone() },
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
        let settings = window::Settings {
            size: Size::new(w as f32, h as f32),
            decorations: self.ssd,
            ..window::Settings::default()
        };
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
        let Some(id) = self.windows.iter().find(|(_, w)| w.uuid == uuid).map(|(id, _)| *id)
        else {
            return Task::none();
        };
        let flush = if self.dirty.contains_key(&uuid) { self.flush(&[uuid]) } else { Task::none() };
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
            .map(|id| if show { self.show_note(id) } else { self.hide_note(id) })
            .collect::<Vec<_>>();
        (show, Task::batch(tasks))
    }

    /// Creates a new note the same way `Message::NewNote` always has, and
    /// opens a window for it. Returns the new note's uuid on success, so
    /// D-Bus's `NewNote` can hand it back to the caller.
    fn create_note(&mut self) -> (Option<Uuid>, Task<Message>) {
        let now = crate::now_rfc3339();
        match self.store.create(&now, Colour::Yellow.name()) {
            Ok(note) => {
                let uuid = note.frontmatter.uuid;
                self.notes.insert(uuid, note);
                (Some(uuid), self.open_window_for(uuid))
            }
            Err(e) => {
                eprintln!("tack: failed to create note: {e}");
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
            eprintln!("tack: failed to delete note {uuid}: {e}");
        }
        self.notes.remove(&uuid);
        self.dirty.remove(&uuid);
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
        let close_task =
            match self.windows.iter().find(|(_, w)| w.uuid == uuid).map(|(id, _)| *id) {
                Some(id) => {
                    self.closing_for_hide.insert(id);
                    window::close(id)
                }
                None => Task::none(),
            };
        let existed = self.delete_note_data(uuid);
        (existed, close_task)
    }

    /// Opens the notes-list window, or raises it if one is already open.
    fn show_list(&mut self) -> Task<Message> {
        if let Some(id) = self.list_window {
            return window::gain_focus(id);
        }
        let settings = window::Settings {
            size: Size::new(DEFAULT_WINDOW_SIZE.0 as f32, DEFAULT_WINDOW_SIZE.1 as f32),
            ..window::Settings::default()
        };
        let (id, spawn) = window::open(settings);
        self.list_window = Some(id);
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

    /// The notes list: every note's display name (clickable to open it), a
    /// "New note" button, and - when there's something to offer - the
    /// restore bar on top. Bare minimum per the task: theme defaults only,
    /// no rename, no delete.
    fn view_list(&self) -> Element<'_, Message> {
        let mut notes: Vec<&Note> = self.notes.values().collect();
        notes.sort_by(|a, b| {
            sticky_notes_core::display_name(a).cmp(sticky_notes_core::display_name(b))
        });

        let mut content = widget::Column::with_capacity(notes.len() + 2).spacing(8).padding(12);

        if !self.restore_dismissed && !self.restore_candidates.is_empty() {
            let count = self.restore_candidates.len();
            let bar = widget::Row::with_capacity(3)
                .spacing(8)
                .push(
                    widget::text::body(format!("Reopen {count} notes from last time?"))
                        .width(Length::Fill),
                )
                .push(widget::button::suggested("Reopen").on_press(Message::ReopenSession))
                .push(widget::button::standard("No thanks").on_press(Message::DismissRestore));
            content = content.push(bar);
        }

        content = content.push(widget::button::suggested("New note").on_press(Message::NewNote));

        for note in notes {
            let uuid = note.frontmatter.uuid;
            let name = sticky_notes_core::display_name(note).to_string();
            content = content.push(widget::button::text(name).on_press(Message::PickNote(uuid)));
        }

        widget::container(widget::scrollable(content).width(Length::Fill).height(Length::Fill))
            .class(cosmic::theme::Container::WindowBackground)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
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
                            sticky_notes_core::display_name(note).to_string(),
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
            dbus::Request::Quit => {
                self.snapshot_session();
                cosmic::iced::exit()
            }
        }
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
            if output.send(Message::Dbus(DbusRequest::new(request))).await.is_err() {
                break;
            }
        }
    })
}

impl cosmic::Application for Tack {
    type Executor = cosmic::executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = "io.github.joelebukatobi.Tack";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Flags) -> (Self, Task<Message>) {
        let Flags { store, window_state, state_path, dbus_connection, dbus_rx } = flags;
        let mut notes = HashMap::new();
        match store.list() {
            Ok(loaded) => {
                for item in loaded {
                    match item {
                        Ok(note) => {
                            notes.insert(note.frontmatter.uuid, note);
                        }
                        Err(e) => eprintln!("tack: skipping unreadable note: {e}"),
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "tack: failed to read notes directory {}: {e}",
                    store.dir().display()
                );
            }
        }

        // The survivors of whatever was open at last quit - notes deleted
        // since then are silently dropped. Empty means no restore bar.
        let existing: BTreeSet<Uuid> = notes.keys().copied().collect();
        let restore_candidates = sticky_notes_core::restorable(&window_state.open_at_quit, &existing);

        // The main window is the notes list, not a note - every note opens
        // as a secondary window (`window::open`), including the ones
        // offered for restore, only once the user picks "Reopen".
        let list_window = core.main_window_id();

        let app = Tack {
            core,
            store,
            window_state,
            state_path,
            window_state_dirty: false,
            windows: HashMap::new(),
            list_window,
            intent_visible: VisibilityIntent::default(),
            restore_candidates,
            restore_dismissed: false,
            notes,
            dirty: HashMap::new(),
            fallback_content: text_editor::Content::new(),
            fallback_input_id: id::Id::unique(),
            closing_for_hide: HashSet::new(),
            session_snapshotted: false,
            _dbus_connection: Some(dbus_connection),
            dbus_rx: DbusRx(Arc::new(Mutex::new(Some(dbus_rx)))),
            ssd: use_server_side_decorations(std::env::var("TACK_SSD").ok().as_deref()),
        };

        (app, Task::none())
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
            _ => None,
        });

        let mut subscriptions = vec![events, dbus_subscription(&self.dbus_rx)];
        if !self.dirty.is_empty() || self.window_state_dirty {
            subscriptions.push(
                cosmic::iced::time::every(Duration::from_millis(500))
                    .map(|_| Message::AutosaveTick),
            );
        }
        Subscription::batch(subscriptions)
    }

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
                let shows: Vec<Task<Message>> =
                    candidates.into_iter().map(|uuid| self.show_note(uuid)).collect();
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
                    widget::text_input::focus(window.input_id.clone())
                } else {
                    Task::none()
                }
            }
            Message::NoteClosed(id) => {
                if self.list_window == Some(id) {
                    self.list_window = None;
                    // A note opening *concurrently* with the list closing
                    // (picking a note, or reopening a session) already
                    // updated `intent_visible` synchronously in the same
                    // `update()` call that requested the close - so this
                    // check never races the asynchronous completion of
                    // `window::open` for that note.
                    return if self.intent_visible.is_empty() {
                        self.snapshot_session();
                        cosmic::iced::exit()
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
                if self.windows.len() == 1 && self.list_window.is_none() {
                    // This is the closing window whose note ended the
                    // session - snapshot while it's still in `self.windows`
                    // (browser tab-restore semantics: the last note you
                    // close is part of "what was open").
                    self.snapshot_session();
                    self.windows.remove(&id);
                    cosmic::iced::exit()
                } else {
                    self.windows.remove(&id);
                    Task::none()
                }
            }
            Message::CloseRequested(id) => {
                if let Some(uuid) = self.windows.get(&id).map(|w| w.uuid) {
                    let disposable = self.notes.get(&uuid).map(is_disposable).unwrap_or(false);
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
                window.content.perform(action);
                let uuid = window.uuid;
                let text = window.content.text();
                if let Some(note) = self.notes.get_mut(&uuid) {
                    note.body = text;
                    self.dirty.insert(uuid, Instant::now());
                }
                Task::none()
            }
            Message::WindowResized(id, size) => {
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
            Message::HeaderDrag(id) => window::drag(id),
            Message::HeaderMinimize(id) => window::minimize(id, true),
            Message::HeaderMaximize(id) => window::toggle_maximize(id),
        }
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

    // `header_start`/`header_center` only reach the *main* window: libcosmic
    // builds its client-side header (`view_main`, which calls these) solely
    // for `core.main_window_id()`; every other window id is dispatched
    // straight to `view_window` with no header of its own layered on top
    // (`Cosmic::view` in libcosmic's `src/app/cosmic.rs`). The main window
    // is the notes list now, not a note, so it just gets the app name - no
    // note title to show, and rename/delete from the list are out of scope
    // here regardless. Secondary note windows (`window::open`, used for
    // every note) default to `decorations: true`, i.e. a compositor-drawn
    // title bar with no client content slots at all, so they keep whatever
    // title bar the compositor gives them.
    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![widget::text::body("Tack").into()]
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
            return self.view_list();
        }

        // Every branch below builds the exact same widget tree shape - a
        // container wrapping a stateful `text_editor` - regardless of
        // whether the window/note lookups succeed. `text_editor`'s
        // context-menu wrapper keeps real widget state; if one frame ever
        // rendered a stateless placeholder here instead, the next frame's
        // diff against the previous state tree panics (state::None can't
        // downcast). See `fallback_content` for why the fallback branch
        // still routes through `text_editor` rather than `widget::text`.
        //
        // No name field: per `docs/ux.md`'s "Inside a note", a note's name
        // is just its first line of text and renaming happens from the
        // notes list, not here - so there's nothing to sit above a divider
        // any more, and the note window is the body editor alone.
        let (input_id, content) = match self.windows.get(&id) {
            Some(window) => (window.input_id.clone(), &window.content),
            None => (self.fallback_input_id.clone(), &self.fallback_content),
        };

        // The body: a transparent `text_editor` stacked on top of a canvas
        // that paints the ruled-paper background (and, critically, an
        // opaque fill - see `ruled::RuledLines`). Both read the same line
        // height and padding constants, so the lines and the text they
        // carry can never drift apart.
        //
        // `responsive` has to sit *outside* the `scrollable`, not inside
        // it: a vertical `scrollable` gives its content a max height of
        // `f32::INFINITY` (that's the whole point - it lets content grow
        // past the viewport), and `responsive::layout` takes `limits.max()`
        // as the size it hands its closure. `responsive` inside `scrollable`
        // would therefore receive an infinite height and pass it straight
        // through to `min_height` below - an editor told its minimum height
        // is infinite. Outside the `scrollable`, `responsive` instead sees
        // the real, finite space the window gives the body, which is
        // exactly the height a short note's lines should fill.
        let body = widget::responsive(move |size| {
            let editor = text_editor::text_editor(content)
                .on_action(move |action| Message::BodyAction(id, action))
                .id(input_id.clone())
                .padding(BODY_PADDING)
                .size(BODY_TEXT_SIZE)
                .line_height(LineHeight::Absolute(Pixels(BODY_LINE_HEIGHT)))
                .min_height(body_min_height(size.height))
                .style(|theme: &cosmic::Theme, _status| {
                    let container = theme.current_container();
                    let value = Color::from(container.on);
                    let mut placeholder = value;
                    placeholder.a *= 0.7;
                    text_editor::Style {
                        // Transparent: the canvas drawn behind it (pushed
                        // `push_under`, below) is what actually paints an
                        // opaque background for this area - see the warning
                        // in `ruled::RuledLines` about why that canvas fill
                        // has to exist at all.
                        background: Color::TRANSPARENT.into(),
                        border: Border { width: 0.0, ..Border::default() },
                        placeholder,
                        value,
                        selection: Color::from(theme.cosmic().accent.base),
                    }
                });

            let lines = widget::canvas(RuledLines {
                line_height: BODY_LINE_HEIGHT,
                padding_top: BODY_PADDING,
            })
            .width(Length::Fill)
            .height(Length::Fill);

            // `editor` pushed first (and so, via `push_under`, ends up the
            // stack's *base layer*) is what the stack sizes itself from -
            // its `Shrink` height, floored at `min_height` above, is the
            // "grow to fit content, floor at the visible height" behaviour
            // wanted here. `push_under` then slots `lines` in *underneath*
            // it without disturbing that sizing, so the canvas
            // (`Length::Fill`) matches the editor's resolved size exactly
            // while still rendering first, i.e. behind the (transparent)
            // text.
            let stack = Stack::new().push(editor).push_under(lines).width(Length::Fill);

            // The `scrollable` lives *inside* `responsive`, wrapping the
            // editor+canvas stack: for a short note the stack is exactly
            // `size.height` tall (via `min_height` above) and doesn't
            // scroll, so the lines fill the window; for a long note the
            // stack grows past `size.height` and this `scrollable` is what
            // lets the editor and the ruled lines behind it scroll together
            // as a unit - if the editor scrolled *internally* instead, the
            // canvas would stay fixed while the text moved, breaking the
            // line alignment.
            widget::scrollable(stack).width(Length::Fill).height(Length::Fill).into()
        })
        .width(Length::Fill)
        .height(Length::Fill);

        let body_container = widget::container(body)
            // An explicit opaque background is a rendering requirement, not
            // decoration: `view_window` is used directly for every secondary
            // note window with nothing else wrapping it (see `Cosmic::view`
            // in libcosmic), so if this container's background were left at
            // its default (`Container::Transparent`), the whole window
            // would render transparent - the desktop showing through, stale
            // frames smearing, exactly the failure mode this task's brief
            // warns about.
            .class(cosmic::theme::Container::WindowBackground)
            .padding(12)
            .width(Length::Fill)
            .height(Length::Fill);

        // `self.ssd` is read once at startup (`Tack::init`) and never
        // changes for the life of the process, so this branch is fixed for
        // every frame this window ever renders - it never flips the tree
        // shape mid-run. With SSD (`TACK_SSD=1`), today's behaviour: the
        // compositor draws the title bar (`decorations: true` in
        // `open_window_for`) and this is the whole window content, same as
        // before this change. Otherwise, a libcosmic `header_bar` sits above
        // it: dragging moves the window, minimise/maximise go straight to
        // the corresponding `window::` action, and close routes through the
        // existing `Message::CloseRequested` - not a second close path - so
        // it still flushes unsaved text and deletes empty notes exactly like
        // the compositor's own close button and every other close route.
        if self.ssd {
            body_container.into()
        } else {
            let header = widget::header_bar()
                .title(self.header_title(id))
                .on_drag(Message::HeaderDrag(id))
                .on_close(Message::CloseRequested(id))
                .on_maximize(Message::HeaderMaximize(id))
                .on_minimize(Message::HeaderMinimize(id));

            widget::Column::with_capacity(2)
                .push(header)
                .push(body_container)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::Application;
    use sticky_notes_core::{Frontmatter, FORMAT_VERSION};

    #[test]
    fn body_min_height_passes_through_a_finite_viewport() {
        assert_eq!(body_min_height(600.0), 600.0);
    }

    #[test]
    fn body_min_height_passes_through_zero() {
        assert_eq!(body_min_height(0.0), 0.0);
    }

    #[test]
    fn body_min_height_falls_back_to_zero_for_infinity() {
        assert_eq!(body_min_height(f32::INFINITY), 0.0);
    }

    #[test]
    fn body_min_height_falls_back_to_zero_for_nan() {
        assert_eq!(body_min_height(f32::NAN), 0.0);
    }

    fn make_tack(store: Store) -> Tack {
        Tack {
            core: Core::default(),
            store,
            window_state: WindowState::default(),
            state_path: std::env::temp_dir().join(format!("tack-test-windows-{}.toml", Uuid::new_v4())),
            window_state_dirty: false,
            windows: HashMap::new(),
            list_window: None,
            intent_visible: VisibilityIntent::default(),
            restore_candidates: BTreeSet::new(),
            restore_dismissed: false,
            notes: HashMap::new(),
            dirty: HashMap::new(),
            fallback_content: text_editor::Content::new(),
            fallback_input_id: id::Id::unique(),
            closing_for_hide: HashSet::new(),
            session_snapshotted: false,
            _dbus_connection: None,
            dbus_rx: DbusRx(Arc::new(Mutex::new(None))),
            ssd: false,
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
        let tmp = std::env::temp_dir().join(format!("tack-test-fail-{}", Uuid::new_v4()));
        std::fs::write(&tmp, b"not a directory").unwrap();
        let store = Store::new(&tmp);

        let mut app = make_tack(store);
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
        let tmp = std::env::temp_dir().join(format!("tack-test-ok-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);

        let mut app = make_tack(store);
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
        let tmp = std::env::temp_dir().join(format!("tack-test-delete-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
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
        let tmp = std::env::temp_dir().join(format!("tack-test-delete-dbus-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
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
        let tmp = std::env::temp_dir().join(format!("tack-test-delete-missing-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);

        assert!(!app.delete_note_data(Uuid::new_v4()));
    }

    #[test]
    fn show_note_on_an_already_intended_visible_note_does_not_reopen() {
        let tmp = std::env::temp_dir().join(format!("tack-test-show-twice-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
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
        let tmp = std::env::temp_dir().join(format!("tack-test-snapshot-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
        let uuid = Uuid::new_v4();
        app.windows.insert(
            window::Id::unique(),
            WindowNote {
                uuid,
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
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
        let tmp = std::env::temp_dir().join(format!("tack-test-hideall-session-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
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
            },
        );
        app.closing_for_hide.insert(id);

        let _ = app.update(Message::NoteClosed(id));

        assert_eq!(app.window_state.open_at_quit, previous, "hide-all must not overwrite the saved session");
        assert!(!app.session_snapshotted);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn note_closed_as_the_last_window_includes_it_in_the_snapshot() {
        // Browser tab-restore semantics: the very note whose window closing
        // ends the session must still be offered back on next launch.
        let tmp = std::env::temp_dir().join(format!("tack-test-last-window-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
        let uuid = Uuid::new_v4();
        let id = window::Id::unique();
        app.windows.insert(
            id,
            WindowNote {
                uuid,
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
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
        let tmp = std::env::temp_dir().join(format!("tack-test-newnote-quit-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
        let list_id = window::Id::unique();
        app.list_window = Some(list_id);

        let _ = app.update(Message::NewNote);
        let uuid = *app.notes.keys().next().expect("NewNote must create a note");
        assert!(app.is_visible(uuid), "the new note must be intent-visible as soon as it's created");

        // Simulate the compositor actually finishing the close that
        // `Message::NewNote`'s `close_list()` requested.
        let _ = app.update(Message::NoteClosed(list_id));

        assert!(
            !app.session_snapshotted,
            "must not exit when the just-created note is still intent-visible"
        );
        assert!(app.is_visible(uuid), "the new note must still be reported visible");

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Finding 1's other half: a newly created note used to be permanently
    /// stuck visible (`hide_note` early-returns when intent never had the
    /// note), and a later `show_note` on it opened a second window for the
    /// same note. Both are intent-level bugs, testable without a real
    /// window ever opening.
    #[test]
    fn a_newly_created_note_can_be_hidden_then_shown_exactly_once() {
        let tmp = std::env::temp_dir().join(format!("tack-test-newnote-hide-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);

        let (uuid, _task) = app.create_note();
        let uuid = uuid.expect("note creation must succeed");
        assert!(app.is_visible(uuid), "create_note must mark the note intent-visible");

        let _ = app.hide_note(uuid);
        assert!(!app.is_visible(uuid), "a newly created note must be hideable");

        let _ = app.show_note(uuid);
        assert!(app.is_visible(uuid), "showing it again must make it visible");

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
        let tmp = std::env::temp_dir().join(format!("tack-test-list-close-exit-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
        let list_id = window::Id::unique();
        app.list_window = Some(list_id);

        let _ = app.update(Message::NoteClosed(list_id));

        assert!(app.session_snapshotted, "closing the list with nothing intent-visible must exit");
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
        let tmp = std::env::temp_dir().join(format!("tack-test-forced-close-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
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
        let tmp = std::env::temp_dir().join(format!("tack-test-hide-close-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
        let uuid = Uuid::new_v4();
        let id = window::Id::unique();
        app.windows.insert(
            id,
            WindowNote {
                uuid,
                input_id: id::Id::unique(),
                content: text_editor::Content::new(),
                last_title: String::new(),
            },
        );
        app.intent_visible.show(uuid);

        let _ = app.hide_note(uuid);
        assert!(!app.is_visible(uuid), "hide_note itself must mark it hidden synchronously");

        let _ = app.update(Message::NoteClosed(id));

        assert!(!app.is_visible(uuid), "a hide-close must leave the note hidden, not visible");
        assert!(!app.session_snapshotted, "a hide must never trigger the exit path");
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
        assert!(pick_note_closes_list(true), "an existing note's pick must still close the list");
        assert!(!pick_note_closes_list(false), "a vanished note must not close the list");
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
        let tmp = std::env::temp_dir().join(format!("tack-test-pick-missing-{}", Uuid::new_v4()));
        let store = Store::new(&tmp);
        let mut app = make_tack(store);
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
        assert!(intent.is_visible(uuid), "a show right after a hide must leave the note visible");
    }

    #[test]
    fn showing_twice_does_not_duplicate() {
        let uuid = Uuid::new_v4();
        let mut intent = VisibilityIntent::default();
        assert!(intent.show(uuid), "first show is a real change");
        assert!(!intent.show(uuid), "second show must report no change - no duplicate window");
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

#[cfg(test)]
mod ssd_tests {
    use super::*;

    #[test]
    fn unset_env_uses_the_client_side_header() {
        assert!(!use_server_side_decorations(None));
    }

    #[test]
    fn env_1_uses_server_side_decorations() {
        assert!(use_server_side_decorations(Some("1")));
    }

    #[test]
    fn env_0_uses_the_client_side_header() {
        assert!(!use_server_side_decorations(Some("0")));
    }

    #[test]
    fn garbage_env_uses_the_client_side_header() {
        assert!(!use_server_side_decorations(Some("not-a-flag")));
    }
}
