use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use cosmic::app::{Core, Task};
use cosmic::iced::widget::container;
use cosmic::iced::{event, window, Color, Length, Subscription};
use cosmic::prelude::*;
use cosmic::widget;
use sticky_notes_core::{Note, Store};
use uuid::Uuid;

use crate::palette::Colour;

/// How long to wait after the last keystroke before writing a note to disk.
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
pub enum Message {
    NewNote,
    NoteOpened(window::Id),
    NoteClosed(window::Id),
    CloseRequested(window::Id),
    BodyEdited(window::Id, String),
    AutosaveTick,
}

pub struct Tack {
    core: Core,
    store: Store,
    /// Which note each open window is showing.
    windows: HashMap<window::Id, Uuid>,
    notes: HashMap<Uuid, Note>,
    /// Notes edited since their last save, and when they were last edited.
    dirty: HashMap<Uuid, Instant>,
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
            if let Some(note) = self.notes.get(id) {
                if let Err(e) = self.store.save(note) {
                    eprintln!("tack: failed to save note {id}: {e}");
                }
            }
            self.dirty.remove(id);
        }
    }

    fn flush_all(&mut self) {
        let ids: Vec<Uuid> = self.dirty.keys().copied().collect();
        self.flush(&ids);
    }

    fn open_window_for(&mut self, uuid: Uuid) -> Task<Message> {
        let (id, spawn) = window::open(window::Settings::default());
        self.windows.insert(id, uuid);
        spawn.map(|id| cosmic::Action::App(Message::NoteOpened(id)))
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
        for loaded in store.list().unwrap_or_default() {
            match loaded {
                Ok(note) => {
                    notes.insert(note.frontmatter.uuid, note);
                }
                Err(e) => eprintln!("tack: skipping unreadable note: {e}"),
            }
        }

        let mut app = Tack {
            core,
            store,
            windows: HashMap::new(),
            notes,
            dirty: HashMap::new(),
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
            Message::NoteOpened(_id) => Task::none(),
            Message::NoteClosed(id) => {
                self.windows.remove(&id);
                Task::none()
            }
            Message::CloseRequested(id) => {
                if let Some(uuid) = self.windows.get(&id).copied() {
                    self.flush(&[uuid]);
                }
                self.windows.remove(&id);
                window::close(id)
            }
            Message::BodyEdited(id, value) => {
                if let Some(uuid) = self.windows.get(&id).copied() {
                    if let Some(note) = self.notes.get_mut(&uuid) {
                        note.body = value;
                        self.dirty.insert(uuid, Instant::now());
                    }
                }
                Task::none()
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
        widget::text::body("tack").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        let Some(note) = self.windows.get(&id).and_then(|uuid| self.notes.get(uuid)) else {
            return widget::text::body("").into();
        };

        let colour = Colour::from_name(&note.frontmatter.color);

        widget::container(
            widget::text_input("", &note.body)
                .on_input(move |value| Message::BodyEdited(id, value)),
        )
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .class(cosmic::theme::Container::custom(move |_theme| {
            let dark = cosmic::theme::is_dark();
            let (br, bg, bb) = colour.background(dark);
            let (tr, tg, tb) = colour.text(dark);
            container::Style {
                background: Some(cosmic::iced::Background::Color(Color::from_rgb8(br, bg, bb))),
                text_color: Some(Color::from_rgb8(tr, tg, tb)),
                ..container::Style::default()
            }
        }))
        .into()
    }
}
