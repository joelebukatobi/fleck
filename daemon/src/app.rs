use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use cosmic::app::{Core, Task};
use cosmic::iced::core::id;
use cosmic::iced::{event, window, Length, Subscription};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::text_editor;
use sticky_notes_core::{is_disposable, Note, Store};
use uuid::Uuid;

use crate::palette::Colour;

/// How long to wait after the last keystroke before writing a note to disk.
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// The window title for a note: its explicit name, or its first non-empty
/// body line, or a sensible fallback for a note with no content yet.
fn window_title(note: &Note) -> String {
    sticky_notes_core::display_name(note).to_string()
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
}

/// A window showing a note: which note it is, the stable id its text
/// editor is registered under (needed to focus it on open), the stable id
/// of its name field, and the editor's own buffer (view state, synced
/// from/to `Note.body`).
struct WindowNote {
    uuid: Uuid,
    input_id: id::Id,
    name_input_id: id::Id,
    content: text_editor::Content,
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
}

impl Tack {
    /// Writes every note that has been dirty for at least the debounce
    /// window through `Store::save`. Returns the ids that were flushed.
    fn flush_due(&mut self, now: Instant) -> HashSet<Uuid> {
        let due: Vec<Uuid> = self
            .dirty
            .iter()
            .filter(|(_, &last_edit)| now.duration_since(last_edit) >= AUTOSAVE_DEBOUNCE)
            .map(|(id, _)| *id)
            .collect();
        self.flush(&due);
        due.into_iter().collect()
    }

    /// Force-writes the given notes through `Store::save`, regardless of the
    /// debounce, and clears their dirty flag.
    fn flush(&mut self, ids: &[Uuid]) {
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
            }
        }
    }

    fn flush_all(&mut self) {
        let ids: Vec<Uuid> = self.dirty.keys().copied().collect();
        self.flush(&ids);
    }

    fn open_window_for(&mut self, uuid: Uuid) -> Task<Message> {
        let (id, spawn) = window::open(window::Settings::default());
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
        self.windows.insert(id, WindowNote { uuid, input_id, name_input_id, content });
        let opened = spawn.map(|id| cosmic::Action::App(Message::NoteOpened(id)));

        let title = self
            .notes
            .get(&uuid)
            .map(window_title)
            .unwrap_or_else(|| sticky_notes_core::UNNAMED.to_string());
        let titled = self.set_window_title(title, id);

        Task::batch([opened, titled])
    }
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
        };

        let uuids: Vec<Uuid> = app.notes.keys().copied().collect();
        let tasks: Vec<Task<Message>> =
            uuids.into_iter().map(|uuid| app.open_window_for(uuid)).collect();

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

        if self.dirty.is_empty() {
            events
        } else {
            Subscription::batch([
                events,
                cosmic::iced::time::every(Duration::from_millis(100))
                    .map(|_| Message::AutosaveTick),
            ])
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewNote => {
                let now = crate::now_rfc3339();
                match self.store.create(&now, Colour::Yellow.name()) {
                    Ok(note) => {
                        let uuid = note.frontmatter.uuid;
                        self.notes.insert(uuid, note);
                        self.open_window_for(uuid)
                    }
                    Err(e) => {
                        eprintln!("tack: failed to create note: {e}");
                        Task::none()
                    }
                }
            }
            Message::NoteOpened(id) => {
                if let Some(window) = self.windows.get(&id) {
                    widget::text_input::focus(window.input_id.clone())
                } else {
                    Task::none()
                }
            }
            Message::NoteClosed(id) => {
                self.windows.remove(&id);
                Task::none()
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
                        self.flush(&[uuid]);
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
                let title = window_title(note);
                self.set_window_title(title, id)
            }
            Message::AutosaveTick => {
                self.flush_due(Instant::now());
                Task::none()
            }
        }
    }

    fn on_app_exit(&mut self) -> Option<Message> {
        self.flush_all();
        None
    }

    fn view(&self) -> Element<'_, Message> {
        // Unreachable: `main.rs` runs this app with `no_main_window(true)`,
        // so `self.core().main_window_id()` is always `None`. libcosmic's
        // `Cosmic::view` dispatches every window id to `view_window` unless
        // it equals the main window id, and `None.is_none_or(..)` is always
        // true, so that comparison never picks this method for any window.
        // A stateless placeholder here used to be a trap: iced calls the
        // trait's `view` too when probing/sizing, and returning a
        // shapeless `text::body` risked the same stateless-tree panic
        // `view_window` used to hit. Fail loudly instead if that ever
        // changes (e.g. `no_main_window` is dropped).
        unreachable!("Tack::view is unreachable under no_main_window(true)")
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

        app.flush(&[id]);

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

        app.flush(&[id]);

        assert!(!app.dirty.contains_key(&id));

        std::fs::remove_dir_all(&tmp).ok();
    }
}
