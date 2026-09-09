use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use cosmic::app::{Core, Task};
use cosmic::iced::core::id;
use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{SinkExt, Stream, StreamExt};
use cosmic::iced::{event, window, Length, Subscription};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::text_editor;
use sticky_notes_core::{is_disposable, Note, Store};
use uuid::Uuid;

use crate::dbus;
use crate::palette::Colour;

/// How long to wait after the last keystroke before writing a note to disk.
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// The window title for a note: its explicit name, or its first non-empty
/// body line, or a sensible fallback for a note with no content yet.
fn window_title(note: &Note) -> String {
    sticky_notes_core::display_name(note).to_string()
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

#[derive(Debug, Clone)]
pub enum Message {
    NewNote,
    NoteOpened(window::Id),
    NoteClosed(window::Id),
    CloseRequested(window::Id),
    BodyAction(window::Id, text_editor::Action),
    NameChanged(window::Id, String),
    AutosaveTick,
    /// A D-Bus call came in and is waiting on `self` to act on it.
    Dbus(DbusRequest),
    /// The D-Bus name is already owned by another running instance - this
    /// process loses the race and quits rather than fight over it.
    DbusUnavailable,
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
/// editor is registered under (needed to focus it on open), the stable id
/// of its name field, the editor's own buffer (view state, synced from/to
/// `Note.body`), and the title last actually sent to the compositor (so it
/// is only re-sent when the display name has changed).
struct WindowNote {
    uuid: Uuid,
    input_id: id::Id,
    name_input_id: id::Id,
    content: text_editor::Content,
    last_title: String,
}

pub struct Tack {
    core: Core,
    store: Store,
    /// Which note each open window is showing.
    windows: HashMap<window::Id, WindowNote>,
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
    /// Stable id for the fallback name field, for the same reason.
    fallback_name_input_id: id::Id,
    /// Windows currently being closed by `hide_note`/`delete_note` rather
    /// than by the user. `NoteClosed` consults this to tell "the last note
    /// window just got hidden" apart from "the user just closed the last
    /// note window" - only the latter should exit the process (see
    /// `Message::NoteClosed`).
    closing_for_hide: HashSet<window::Id>,
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
        self.flush(&due)
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
    }

    /// Registers `uuid` as the note shown by window `id` (already open -
    /// either the main window, claimed once at startup, or a window just
    /// returned by `window::open`) and pushes its initial title.
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
        let name_input_id = id::Id::unique();
        let body = self.notes.get(&uuid).map(|note| note.body.as_str()).unwrap_or("");
        let content = text_editor::Content::with_text(body);
        let title = self
            .notes
            .get(&uuid)
            .map(window_title)
            .unwrap_or_else(|| sticky_notes_core::UNNAMED.to_string());
        self.windows.insert(
            id,
            WindowNote { uuid, input_id, name_input_id, content, last_title: title.clone() },
        );
        self.set_window_title(title, id)
    }

    /// Opens a brand-new secondary window for `uuid`. The first note at
    /// startup does not go through this - it attaches to the main window
    /// libcosmic already created (see `init`), since a window can't be
    /// opened twice.
    fn open_window_for(&mut self, uuid: Uuid) -> Task<Message> {
        let (id, spawn) = window::open(window::Settings::default());
        let registered = self.register_window(id, uuid);
        let opened = spawn.map(|id| cosmic::Action::App(Message::NoteOpened(id)));

        Task::batch([opened, registered])
    }

    /// Whether `uuid` currently has an open window. This *is* the
    /// visibility model - a hidden note is simply one with no window, still
    /// sitting in `self.notes` exactly as it was.
    fn is_visible(&self, uuid: Uuid) -> bool {
        self.windows.values().any(|w| w.uuid == uuid)
    }

    /// Closes the window showing `uuid`, if it has one open, without
    /// touching the note itself: any pending edit is flushed first, then
    /// the window is closed directly through `window::close` rather than
    /// going via `Message::CloseRequested` - so the delete-on-close rule for
    /// empty notes never runs. The window id is recorded in
    /// `closing_for_hide` so the `NoteClosed` that follows knows this
    /// closure isn't the user quitting.
    fn hide_note(&mut self, uuid: Uuid) -> Task<Message> {
        let Some(id) = self.windows.iter().find(|(_, w)| w.uuid == uuid).map(|(id, _)| *id)
        else {
            return Task::none();
        };
        let flush = if self.dirty.contains_key(&uuid) { self.flush(&[uuid]) } else { Task::none() };
        self.closing_for_hide.insert(id);
        Task::batch([flush, window::close(id)])
    }

    /// Opens a window for `uuid` from its stored content, unless it already
    /// has one. A no-op for a uuid that isn't a known note at all.
    fn show_note(&mut self, uuid: Uuid) -> Task<Message> {
        if self.is_visible(uuid) || !self.notes.contains_key(&uuid) {
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

    /// Deletes `uuid` through `Store::delete`, closing its window first (if
    /// it has one) exactly like `hide_note` - directly, bypassing the
    /// delete-on-close check, since this delete is already unconditional.
    /// Returns whether the note existed.
    fn delete_note(&mut self, uuid: Uuid) -> (bool, Task<Message>) {
        if !self.notes.contains_key(&uuid) {
            return (false, Task::none());
        }
        self.dirty.remove(&uuid);
        let close_task =
            match self.windows.iter().find(|(_, w)| w.uuid == uuid).map(|(id, _)| *id) {
                Some(id) => {
                    self.closing_for_hide.insert(id);
                    window::close(id)
                }
                None => Task::none(),
            };
        if let Err(e) = self.store.delete(uuid) {
            eprintln!("tack: failed to delete note {uuid}: {e}");
        }
        self.notes.remove(&uuid);
        (true, close_task)
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
            dbus::Request::Quit => cosmic::iced::exit(),
        }
    }
}

/// Runs the `io.github.joelebukatobi.Tack` D-Bus service and forwards each
/// incoming call into the application as a `Message::Dbus`. Built with
/// `Subscription::run` (a plain `fn`, per its signature) rather than a
/// closure, so it identifies the same recipe across every `subscription()`
/// call instead of being torn down and restarted each frame.
fn dbus_subscription() -> Subscription<Message> {
    Subscription::run(dbus_worker)
}

fn dbus_worker() -> impl Stream<Item = Message> {
    cosmic::iced::stream::channel(16, async move |mut output| {
        let (tx, mut rx) = mpsc::channel::<dbus::Request>(16);

        let connection = zbus::connection::Builder::session()
            .and_then(|b| b.serve_at(dbus::OBJECT_PATH, dbus::TackInterface::new(tx)))
            .and_then(|b| b.name(dbus::SERVICE_NAME));
        let connection = match connection {
            Ok(builder) => builder.build().await,
            Err(e) => Err(e),
        };

        // Kept alive for as long as this subscription runs: dropping it
        // would release the well-known name and tear the service down.
        let _connection = match connection {
            Ok(conn) => conn,
            Err(e) => {
                // Either another `tack` instance already owns the name, or
                // the session bus itself isn't reachable. Either way, this
                // process has no D-Bus service to offer - say so once, and
                // let the application decide whether that's fatal.
                eprintln!("tack: not starting the D-Bus service: {e}");
                let _ = output.send(Message::DbusUnavailable).await;
                std::future::pending::<()>().await;
                unreachable!("pending future never resolves");
            }
        };

        while let Some(request) = rx.next().await {
            if output.send(Message::Dbus(DbusRequest::new(request))).await.is_err() {
                break;
            }
        }
    })
}

impl cosmic::Application for Tack {
    type Executor = cosmic::executor::Default;
    type Flags = Store;
    type Message = Message;

    const APP_ID: &'static str = "io.github.joelebukatobi.Tack";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, store: Store) -> (Self, Task<Message>) {
        let mut notes = HashMap::new();
        // Only true when `Store::list` succeeded and returned zero entries.
        // A `list` failure (already logged below) must NOT trigger note
        // creation: the notes may be on disk and merely unreadable, and
        // creating a new one then would look like data loss.
        let mut store_is_empty = false;
        match store.list() {
            Ok(loaded) => {
                store_is_empty = loaded.is_empty();
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

        // Never launch with zero windows and no way to create a note except
        // a terminal flag: if the store is genuinely empty, seed it with one.
        if store_is_empty {
            match store.create(&crate::now_rfc3339(), crate::palette::Colour::Yellow.name()) {
                Ok(note) => {
                    notes.insert(note.frontmatter.uuid, note);
                }
                Err(e) => eprintln!("tack: failed to create initial note: {e}"),
            }
        }

        let mut app = Tack {
            core,
            store,
            windows: HashMap::new(),
            notes,
            dirty: HashMap::new(),
            fallback_content: text_editor::Content::new(),
            fallback_input_id: id::Id::unique(),
            fallback_name_input_id: id::Id::unique(),
            closing_for_hide: HashSet::new(),
        };

        // The seeded (or loaded) notes above must exist before this: the
        // main window always gets the first one, `open_window_for` opens a
        // fresh secondary window for every other note.
        let mut uuids: Vec<Uuid> = app.notes.keys().copied().collect();
        let mut tasks = Vec::new();
        if let Some(main_id) = app.core.main_window_id() {
            if let Some(first) = uuids.pop() {
                tasks.push(app.register_window(main_id, first));
            }
        }
        for uuid in uuids {
            tasks.push(app.open_window_for(uuid));
        }

        (app, Task::batch(tasks))
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
            _ => None,
        });

        let mut subscriptions = vec![events, dbus_subscription()];
        if !self.dirty.is_empty() {
            subscriptions.push(
                cosmic::iced::time::every(Duration::from_millis(500))
                    .map(|_| Message::AutosaveTick),
            );
        }
        Subscription::batch(subscriptions)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewNote => self.create_note().1,
            Message::NoteOpened(id) => {
                if let Some(window) = self.windows.get(&id) {
                    widget::text_input::focus(window.input_id.clone())
                } else {
                    Task::none()
                }
            }
            Message::NoteClosed(id) => {
                // A window closed *because it was hidden* (or deleted) is
                // not the user quitting: `hide_all` deliberately drives
                // `self.windows` to empty and the process must stay alive
                // for a later `show`/`toggle-all` to reopen it (spec's
                // hide-all requirement wins here over Task 11's
                // exit-when-last-window-closes rule - see the task report
                // for why). Only a window that closed on its own account -
                // the user's X button, via `CloseRequested` - counts toward
                // "the last note window is gone, exit".
                let was_hide = self.closing_for_hide.remove(&id);
                self.windows.remove(&id);
                if self.windows.is_empty() && !was_hide {
                    // The main window is just the first note now, not
                    // special - exit once the *last* note window (main or
                    // secondary) is gone, not tied to which one it was.
                    cosmic::iced::exit()
                } else {
                    Task::none()
                }
            }
            Message::CloseRequested(id) => {
                if let Some(uuid) = self.windows.get(&id).map(|w| w.uuid) {
                    let disposable = self.notes.get(&uuid).map(is_disposable).unwrap_or(false);
                    if disposable {
                        if let Err(e) = self.store.delete(uuid) {
                            eprintln!("tack: failed to delete empty note {uuid}: {e}");
                        }
                        self.notes.remove(&uuid);
                        self.dirty.remove(&uuid);
                    } else if self.dirty.contains_key(&uuid) {
                        let _ = self.flush(&[uuid]);
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
            Message::NameChanged(id, name) => {
                let Some(uuid) = self.windows.get(&id).map(|w| w.uuid) else {
                    return Task::none();
                };
                let Some(note) = self.notes.get_mut(&uuid) else {
                    return Task::none();
                };
                note.frontmatter.name = name;
                self.dirty.insert(uuid, Instant::now());
                // Deliberately not setting the window title here: doing so
                // on every keystroke is a compositor round-trip per
                // character. `flush`/`flush_due` sync the title instead, at
                // most once per autosave debounce.
                Task::none()
            }
            Message::AutosaveTick => self.flush_due(Instant::now()),
            Message::Dbus(request) => match request.take() {
                Some(request) => self.handle_dbus_request(request),
                None => Task::none(),
            },
            Message::DbusUnavailable => {
                eprintln!(
                    "tack: another instance already owns {}; exiting",
                    dbus::SERVICE_NAME
                );
                cosmic::iced::exit()
            }
        }
    }

    fn on_app_exit(&mut self) -> Option<Message> {
        self.flush_all();
        None
    }

    fn view(&self) -> Element<'_, Message> {
        // `Cosmic::view` dispatches every window id except the main one to
        // `view_window` directly; for the main window it falls back to
        // this method. Since the main window shows a note like any other,
        // just render it the same way.
        self.view_window(self.core.main_window_id().unwrap())
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        // Every branch below builds the exact same widget tree shape - a
        // container wrapping a name field above a stateful `text_editor` -
        // regardless of whether the window/note lookups succeed.
        // `text_editor`'s context-menu wrapper keeps real widget state; if
        // one frame ever rendered a stateless placeholder here instead, the
        // next frame's diff against the previous state tree panics
        // (state::None can't downcast). See `fallback_content` for why the
        // fallback branch still routes through `text_editor` rather than
        // `widget::text`.
        let (name, name_input_id, input_id, content) = match self.windows.get(&id) {
            Some(window) => {
                let name = self
                    .notes
                    .get(&window.uuid)
                    // The note can be gone while the window is still
                    // technically open (deleted-on-close, pending the real
                    // `Closed` event) - fall back to an empty name but keep
                    // rendering the window's own real editor content.
                    .map(|note| note.frontmatter.name.as_str())
                    .unwrap_or("");
                (name, window.name_input_id.clone(), window.input_id.clone(), &window.content)
            }
            None => (
                "",
                self.fallback_name_input_id.clone(),
                self.fallback_input_id.clone(),
                &self.fallback_content,
            ),
        };

        widget::container(
            cosmic::iced::widget::Column::new()
                .push(
                    widget::text_input("Name", name)
                        .on_input(move |name| Message::NameChanged(id, name))
                        .id(name_input_id),
                )
                .push(
                    text_editor::text_editor(content)
                        .on_action(move |action| Message::BodyAction(id, action))
                        .id(input_id)
                        .height(Length::Fill),
                ),
        )
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sticky_notes_core::{Frontmatter, FORMAT_VERSION};

    fn make_tack(store: Store) -> Tack {
        Tack {
            core: Core::default(),
            store,
            windows: HashMap::new(),
            notes: HashMap::new(),
            dirty: HashMap::new(),
            fallback_content: text_editor::Content::new(),
            fallback_input_id: id::Id::unique(),
            fallback_name_input_id: id::Id::unique(),
            closing_for_hide: HashSet::new(),
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
    fn title_needs_update_when_current_differs_from_last_set() {
        assert!(title_needs_update("New name", "Old name"));
    }

    #[test]
    fn title_no_update_when_current_matches_last_set() {
        assert!(!title_needs_update("Same name", "Same name"));
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
}
