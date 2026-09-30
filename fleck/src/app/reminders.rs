//! The reminders half of the list window: the list itself, and the form for
//! adding or editing one. Reminders are their own items (see
//! `fleck_core::reminder`); one made from a note's menu points at that note.

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime, Timelike};
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;
use fleck_core::{display_name, local_at, Reminder, Repeat};
use uuid::Uuid;

use super::list::{
    CARD_ACTION_PADDING, CARD_CONTENT_SPACING, CARD_HEADING_PADDING_X, CARD_HEADING_PADDING_Y,
    CARD_HEADING_ROW_SPACING, CARD_PADDING, CARD_SPACING,
};
use super::style::{
    card_button_class, card_heading_style, tinted_icon_button_class, IconHoverRole,
};
use super::{Fleck, Message};
use crate::palette::Colour;

/// The date and time formats the form's fields use.
const DATE_FORMAT: &str = "%Y-%m-%d";
const TIME_FORMAT: &str = "%H:%M";
/// How far ahead a new reminder is set by default.
const DEFAULT_LEAD_HOURS: i64 = 1;

/// Which half of the list window is showing. `pub(crate)` only because it
/// rides through `Message`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ListView {
    #[default]
    Notes,
    Reminders,
}

/// A reminder being added or edited. Kept as the text the fields hold, so a
/// half-typed date is not thrown away while the user is still typing.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ReminderForm {
    /// The reminder being edited, or `None` when adding a new one.
    pub(super) uuid: Option<Uuid>,
    pub(super) text: String,
    pub(super) date: String,
    pub(super) time: String,
    pub(super) repeat: Repeat,
    pub(super) note: Option<Uuid>,
}

impl ReminderForm {
    /// A new reminder, due an hour from now, optionally about `note`.
    pub(super) fn new(note: Option<Uuid>, text: String) -> Self {
        let due = Local::now() + chrono::Duration::hours(DEFAULT_LEAD_HOURS);
        Self {
            uuid: None,
            text,
            date: due.format(DATE_FORMAT).to_string(),
            time: due.format(TIME_FORMAT).to_string(),
            repeat: Repeat::Once,
            note,
        }
    }

    /// The form filled in from an existing reminder.
    pub(super) fn editing(reminder: &Reminder) -> Self {
        let due = reminder.due_at().unwrap_or_else(Local::now);
        Self {
            uuid: Some(reminder.uuid),
            text: reminder.text.clone(),
            date: due.format(DATE_FORMAT).to_string(),
            time: due.format(TIME_FORMAT).to_string(),
            repeat: reminder.repeat,
            note: reminder.note,
        }
    }

    /// When the form's date and time say, or `None` while they don't read as
    /// a real moment - which is what greys out Save.
    pub(super) fn due(&self) -> Option<DateTime<Local>> {
        let date = NaiveDate::parse_from_str(self.date.trim(), DATE_FORMAT).ok()?;
        let time = NaiveTime::parse_from_str(self.time.trim(), TIME_FORMAT).ok()?;
        local_at(
            date.year(),
            date.month(),
            date.day(),
            time.hour(),
            time.minute(),
        )
    }

    /// The reminder this form describes, keeping the id when editing one.
    pub(super) fn build(&self) -> Option<Reminder> {
        let due = self.due()?;
        let mut reminder = Reminder::new(self.text.trim().to_string(), due, self.repeat, self.note);
        if let Some(uuid) = self.uuid {
            reminder.uuid = uuid;
        }
        Some(reminder)
    }
}

/// When a reminder is due, in words: the time for today, the weekday and time
/// within a week, otherwise the date.
pub(super) fn format_due(due: DateTime<Local>, now: DateTime<Local>) -> String {
    let time = due.format(TIME_FORMAT).to_string();
    if due.date_naive() == now.date_naive() {
        crate::fl!("due-today", time = time)
    } else if due.date_naive() == (now + chrono::Duration::days(1)).date_naive() {
        crate::fl!("due-tomorrow", time = time)
    } else if due > now && due < now + chrono::Duration::days(7) {
        let weekday = due.format("%a").to_string();
        crate::fl!("due-weekday", weekday = weekday, time = time)
    } else {
        let date = due.format("%e %b %Y").to_string().trim().to_string();
        crate::fl!("due-date", date = date, time = time)
    }
}

pub(super) fn repeat_label(repeat: Repeat) -> String {
    match repeat {
        Repeat::Once => crate::fl!("repeat-once"),
        Repeat::Daily => crate::fl!("repeat-daily"),
        Repeat::Weekdays => crate::fl!("repeat-weekdays"),
        Repeat::Weekly => crate::fl!("repeat-weekly"),
        Repeat::Monthly => crate::fl!("repeat-monthly"),
    }
}

impl Fleck {
    /// What a reminder is called in the list: its own text, or the name of the
    /// note it is about.
    pub(super) fn reminder_title(&self, reminder: &Reminder) -> String {
        if !reminder.text.trim().is_empty() {
            return reminder.text.clone();
        }
        reminder
            .note
            .and_then(|uuid| self.notes.get(&uuid))
            .map_or_else(
                || crate::fl!("reminder-untitled"),
                |note| display_name(note).to_string(),
            )
    }

    /// The reminders list: one card each, soonest first.
    pub(super) fn view_reminders(&self) -> Element<'_, Message> {
        let now = Local::now();
        let reminders = self.reminders.sorted();
        if reminders.is_empty() {
            return widget::container(widget::text::body(crate::fl!("reminders-empty")))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        }
        let mut cards = widget::Column::with_capacity(reminders.len()).spacing(CARD_SPACING);
        for reminder in reminders {
            cards = cards.push(self.view_reminder_card(reminder, now));
        }
        widget::scrollable(cards)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// One reminder's card: when it is due, what it is about, and how often it
    /// repeats, with a delete button. Clicking it opens it for editing.
    fn view_reminder_card(
        &self,
        reminder: &Reminder,
        now: DateTime<Local>,
    ) -> Element<'_, Message> {
        // A reminder about a note wears that note's colour, so the two read as
        // the same thing; a standalone one uses the theme's.
        let colour = reminder
            .note
            .and_then(|uuid| self.notes.get(&uuid))
            .map_or(Colour::Default, |note| {
                Colour::from_name(&note.frontmatter.color)
            });
        let heading_text = colour.heading_text();
        let text_class = |text: Option<(u8, u8, u8)>| {
            text.map_or(cosmic::theme::Text::Default, |text| {
                cosmic::theme::Text::Color(super::style::rgb(text))
            })
        };

        let heading = widget::container(
            widget::Row::with_capacity(2)
                .spacing(CARD_HEADING_ROW_SPACING)
                .align_y(Alignment::Center)
                .push(
                    widget::text::heading(self.reminder_title(reminder))
                        .class(text_class(heading_text))
                        .width(Length::Fill),
                )
                .push(
                    widget::button::icon(crate::icons::trash())
                        .extra_small()
                        .padding(CARD_ACTION_PADDING)
                        .on_press(Message::ReminderDelete(reminder.uuid))
                        .class(tinted_icon_button_class(
                            IconHoverRole::Destructive,
                            heading_text.map(super::style::rgb),
                        )),
                ),
        )
        .class(cosmic::theme::Container::custom(card_heading_style(colour)))
        .padding([
            CARD_HEADING_PADDING_Y,
            0,
            CARD_HEADING_PADDING_Y,
            CARD_HEADING_PADDING_X,
        ])
        .width(Length::Fill);

        let due = reminder.due_at().map_or_else(
            || crate::fl!("reminder-unreadable"),
            |due| format_due(due, now),
        );
        let mut details = widget::Column::with_capacity(2).spacing(CARD_CONTENT_SPACING);
        details = details.push(widget::text::body(due).class(text_class(colour.text())));
        if reminder.repeat != Repeat::Once {
            details = details.push(
                widget::text::caption(repeat_label(reminder.repeat))
                    .class(text_class(colour.text())),
            );
        }

        widget::button::custom(
            widget::Column::with_capacity(2).push(heading).push(
                widget::container(details)
                    .padding(CARD_PADDING)
                    .width(Length::Fill),
            ),
        )
        .padding(0)
        .width(Length::Fill)
        .class(card_button_class(colour))
        .on_press(Message::ReminderEditStart(reminder.uuid))
        .into()
    }

    /// The add-or-edit reminder dialog, over the list window.
    pub(super) fn view_reminder_form(&self, form: &ReminderForm) -> Element<'_, Message> {
        let field = |label: String, value: &str, on_input: fn(String) -> Message| {
            widget::Column::with_capacity(2)
                .spacing(FORM_LABEL_GAP)
                .push(widget::text::caption(label))
                .push(widget::text_input("", value.to_string()).on_input(on_input))
        };

        let mut repeats = widget::Column::with_capacity(Repeat::ALL.len()).spacing(FORM_LABEL_GAP);
        for repeat in Repeat::ALL {
            repeats = repeats.push(widget::radio(
                widget::text::body(repeat_label(repeat)),
                repeat,
                Some(form.repeat),
                Message::ReminderFormRepeat,
            ));
        }

        let mut controls = widget::Column::with_capacity(5)
            .spacing(FORM_FIELD_GAP)
            .push(field(
                crate::fl!("reminder-text"),
                &form.text,
                Message::ReminderFormText,
            ))
            .push(
                widget::Row::with_capacity(2)
                    .spacing(FORM_FIELD_GAP)
                    .push(field(
                        crate::fl!("reminder-date"),
                        &form.date,
                        Message::ReminderFormDate,
                    ))
                    .push(field(
                        crate::fl!("reminder-time"),
                        &form.time,
                        Message::ReminderFormTime,
                    )),
            )
            .push(
                widget::Column::with_capacity(2)
                    .spacing(FORM_LABEL_GAP)
                    .push(widget::text::caption(crate::fl!("reminder-repeat")))
                    .push(repeats),
            );
        if form.due().is_none() {
            controls = controls.push(widget::text::body(crate::fl!("reminder-invalid")));
        }

        let title = if form.uuid.is_some() {
            crate::fl!("reminder-edit-title")
        } else {
            crate::fl!("reminder-new-title")
        };
        let mut save = widget::button::suggested(crate::fl!("save"));
        if form.due().is_some() {
            save = save.on_press(Message::ReminderSave);
        }
        widget::dialog()
            .title(title)
            .width(Length::Fixed(super::list::dialog_width(
                self.list_window_width,
            )))
            .control(controls)
            .primary_action(save)
            .secondary_action(
                widget::button::standard(crate::fl!("cancel")).on_press(Message::ReminderCancel),
            )
            .into()
    }
}

const FORM_FIELD_GAP: u16 = 16;
const FORM_LABEL_GAP: u16 = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_form_is_due_about_an_hour_from_now() {
        let form = ReminderForm::new(None, String::new());
        let due = form.due().expect("the default form is valid");
        let ahead = due - Local::now();
        assert!(ahead > chrono::Duration::minutes(50) && ahead <= chrono::Duration::hours(1));
        assert_eq!(form.uuid, None);
    }

    #[test]
    fn a_half_typed_date_is_simply_not_due_yet() {
        let mut form = ReminderForm::new(None, String::new());
        form.date = "2026-1".to_string();
        assert_eq!(form.due(), None, "Save stays out of reach until it reads");
        assert_eq!(form.build(), None);
    }

    #[test]
    fn editing_keeps_the_reminders_id_and_its_note() {
        let note = Some(Uuid::from_u128(7));
        let original = Reminder::new(
            "water the plants".into(),
            local_at(2026, 10, 1, 9, 0).unwrap(),
            Repeat::Weekly,
            note,
        );

        let mut form = ReminderForm::editing(&original);
        form.time = "10:30".to_string();
        let edited = form.build().expect("a valid form");

        assert_eq!(edited.uuid, original.uuid, "editing replaces, not adds");
        assert_eq!(edited.note, note);
        assert_eq!(edited.repeat, Repeat::Weekly);
        assert_eq!(edited.due_at().unwrap().hour(), 10);
        assert_eq!(edited.due_at().unwrap().minute(), 30);
    }
}
