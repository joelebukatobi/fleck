use std::collections::HashMap;

use cosmic::app::{Core, Task};
use cosmic::iced::{event, window, Subscription};
use cosmic::prelude::*;
use cosmic::widget;
use sticky_notes_core::{Note, Store};
use uuid::Uuid;

use crate::palette::Colour;

#[derive(Debug, Clone)]
pub enum Message {
    NewNote,
    NoteOpened(window::Id),
    NoteClosed(window::Id),
    CloseRequested(window::Id),
    BodyEdited(window::Id, String),
}

pub struct Tack {
    core: Core,
    store: Store,
    /// Which note each open window is showing.
    windows: HashMap<window::Id, Uuid>,
    notes: HashMap<Uuid, Note>,
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

        let app = Tack { core, store, windows: HashMap::new(), notes };
        (app, Task::none())
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, _, id| match event {
            cosmic::iced::Event::Window(window::Event::Opened { .. }) => {
                Some(Message::NoteOpened(id))
            }
            cosmic::iced::Event::Window(window::Event::Closed) => Some(Message::NoteClosed(id)),
            cosmic::iced::Event::Window(window::Event::CloseRequested) => {
                Some(Message::CloseRequested(id))
            }
            _ => None,
        })
    }

    fn view(&self) -> Element<'_, Message> {
        widget::text::body("tack").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        let Some(note) = self.windows.get(&id).and_then(|uuid| self.notes.get(uuid)) else {
            return widget::text::body("").into();
        };

        let colour = Colour::from_name(&note.frontmatter.color);
        let _ = colour;

        widget::container(
            widget::text_input("", &note.body)
                .on_input(move |value| Message::BodyEdited(id, value)),
        )
        .padding(12)
        .width(cosmic::iced::Length::Fill)
        .height(cosmic::iced::Length::Fill)
        .class(cosmic::theme::Container::Primary)
        .into()
    }
}
