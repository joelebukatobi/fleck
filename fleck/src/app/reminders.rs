//! The reminders half of the list window: the list itself, and the form for
//! adding or editing one. Reminders are their own items (see
//! `fleck_core::reminder`); one made from a note's menu points at that note.

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime, Timelike};
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::text_editor;
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
/// How far ahead a new reminder is set by default, rounded up to the next
/// `DEFAULT_ROUNDING` minutes. Short, because "remind me shortly" is the
/// common case and a wrong default is easy to accept without reading.
const DEFAULT_LEAD_MINUTES: i64 = 10;
const DEFAULT_ROUNDING: i64 = 5;

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
    pub(super) description: String,
    pub(super) event: bool,
    pub(super) location: String,
    pub(super) date: String,
    pub(super) time: String,
    pub(super) repeat: Repeat,
    pub(super) note: Option<Uuid>,
}

impl ReminderForm {
    /// A new reminder, due an hour from now, optionally about `note`.
    pub(super) fn new(note: Option<Uuid>, text: String) -> Self {
        let due = default_due();
        Self {
            uuid: None,
            text,
            description: String::new(),
            event: false,
            location: String::new(),
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
            description: reminder.description.clone(),
            event: reminder.event,
            location: reminder.location.clone(),
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
        reminder.description = self.description.trim().to_string();
        reminder.event = self.event;
        // A location belongs to an event; unticking the box lets it go rather
        // than keeping a hidden value that reappears later.
        reminder.location = if self.event {
            self.location.trim().to_string()
        } else {
            String::new()
        };
        if let Some(uuid) = self.uuid {
            reminder.uuid = uuid;
        }
        Some(reminder)
    }
}

/// A new reminder's time: `DEFAULT_LEAD_MINUTES` from now, rounded up to the
/// next `DEFAULT_ROUNDING` minutes so it reads as a round time.
fn default_due() -> DateTime<Local> {
    let soon = Local::now() + chrono::Duration::minutes(DEFAULT_LEAD_MINUTES);
    let over = i64::from(soon.minute()) % DEFAULT_ROUNDING;
    let rounded = soon + chrono::Duration::minutes((DEFAULT_ROUNDING - over) % DEFAULT_ROUNDING);
    rounded.with_second(0).unwrap_or(rounded)
}

/// How far away a time is, in words, for the reminder form: "in 10 minutes",
/// "in 3 hours", "2 days ago".
pub(super) fn relative_to_now(due: DateTime<Local>, now: DateTime<Local>) -> String {
    let ahead = due - now;
    let past = ahead < chrono::Duration::zero();
    let ahead = if past { -ahead } else { ahead };
    let (count, unit) = if ahead < chrono::Duration::hours(1) {
        (ahead.num_minutes().max(1), "minute")
    } else if ahead < chrono::Duration::days(1) {
        (ahead.num_hours(), "hour")
    } else {
        (ahead.num_days(), "day")
    };
    // A number, not a string: Fluent picks singular or plural from it.
    match (past, unit) {
        (false, "minute") => crate::fl!("in-minutes", count = count),
        (false, "hour") => crate::fl!("in-hours", count = count),
        (false, _) => crate::fl!("in-days", count = count),
        (true, "minute") => crate::fl!("ago-minutes", count = count),
        (true, "hour") => crate::fl!("ago-hours", count = count),
        (true, _) => crate::fl!("ago-days", count = count),
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
        let mut details = widget::Column::with_capacity(4).spacing(CARD_CONTENT_SPACING);
        details = details.push(widget::text::body(due).class(text_class(colour.text())));
        if reminder.event && !reminder.location.trim().is_empty() {
            details = details.push(
                widget::text::body(reminder.location.clone()).class(text_class(colour.text())),
            );
        }
        if !reminder.description.trim().is_empty() {
            details = details.push(
                widget::text::caption(reminder.description.clone())
                    .class(text_class(colour.text())),
            );
        }
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
    pub(super) fn view_reminder_form<'a>(
        &'a self,
        form: &'a ReminderForm,
        description: &'a text_editor::Content,
    ) -> Element<'a, Message> {
        let field = |label: String, value: &str, on_input: fn(String) -> Message| {
            widget::Column::with_capacity(2)
                .spacing(FORM_LABEL_GAP)
                .push(widget::text::caption(label))
                .push(widget::text_input("", value.to_string()).on_input(on_input))
        };

        // Two columns: five repeats in one column made the dialog tall and
        // left half of it empty.
        let per_column = Repeat::ALL.len().div_ceil(REPEAT_COLUMNS);
        let mut repeats = widget::Row::with_capacity(REPEAT_COLUMNS).spacing(FORM_FIELD_GAP);
        for column in Repeat::ALL.chunks(per_column) {
            let mut choices = widget::Column::with_capacity(per_column).spacing(FORM_LABEL_GAP);
            for &repeat in column {
                choices = choices.push(widget::radio(
                    widget::text::body(repeat_label(repeat)),
                    repeat,
                    Some(form.repeat),
                    Message::ReminderFormRepeat,
                ));
            }
            repeats = repeats.push(choices.width(Length::Fill));
        }

        let mut controls = widget::Column::with_capacity(7)
            .spacing(FORM_FIELD_GAP)
            .push(field(
                crate::fl!("reminder-text"),
                &form.text,
                Message::ReminderFormText,
            ))
            .push(
                widget::Column::with_capacity(2)
                    .spacing(FORM_LABEL_GAP)
                    .push(widget::text::caption(crate::fl!("reminder-description")))
                    .push(
                        text_editor::text_editor(description)
                            .height(Length::Fixed(DESCRIPTION_HEIGHT))
                            .on_action(Message::ReminderFormDescription),
                    ),
            )
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
            )
            .push(
                widget::checkbox(form.event)
                    .label(crate::fl!("reminder-is-event"))
                    .on_toggle(Message::ReminderFormEvent),
            );
        // The location only matters for an event, so it only appears for one.
        if form.event {
            controls = controls.push(field(
                crate::fl!("reminder-location"),
                &form.location,
                Message::ReminderFormLocation,
            ));
        }
        // Says how far away the typed time is, so an accepted default can't be
        // misread as "soon".
        match form.due() {
            Some(due) => {
                controls = controls.push(widget::text::body(relative_to_now(due, Local::now())));
            }
            None => controls = controls.push(widget::text::body(crate::fl!("reminder-invalid"))),
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
/// The description box's height: a few lines, not a whole page.
const DESCRIPTION_HEIGHT: f32 = 96.0;
/// The repeat choices are laid out in this many columns.
const REPEAT_COLUMNS: usize = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_form_is_due_shortly_on_a_round_five_minutes() {
        let form = ReminderForm::new(None, String::new());
        let due = form.due().expect("the default form is valid");
        let ahead = due - Local::now();

        assert!(
            ahead >= chrono::Duration::minutes(9) && ahead <= chrono::Duration::minutes(15),
            "about ten minutes ahead, not an hour: {ahead}"
        );
        assert_eq!(due.minute() % 5, 0, "rounded to a round five minutes");
        assert_eq!(form.uuid, None);
    }

    #[test]
    fn how_far_away_a_time_is_reads_plainly() {
        /// Without Fluent's bidi isolation marks around the number.
        fn plain(text: &str) -> String {
            text.replace(['\u{2068}', '\u{2069}'], "")
        }
        let now = Local::now();
        assert_eq!(
            plain(&relative_to_now(now + chrono::Duration::minutes(10), now)),
            "in 10 minutes"
        );
        assert_eq!(
            plain(&relative_to_now(now + chrono::Duration::hours(3), now)),
            "in 3 hours"
        );
        assert_eq!(
            plain(&relative_to_now(now - chrono::Duration::days(2), now)),
            "2 days ago"
        );
        assert_eq!(
            plain(&relative_to_now(now + chrono::Duration::hours(1), now)),
            "in 1 hour",
            "singular reads properly"
        );
    }

    #[test]
    fn a_half_typed_date_is_simply_not_due_yet() {
        let mut form = ReminderForm::new(None, String::new());
        form.date = "2026-1".to_string();
        assert_eq!(form.due(), None, "Save stays out of reach until it reads");
        assert_eq!(form.build(), None);
    }

    #[test]
    fn unticking_event_lets_its_location_go() {
        let mut form = ReminderForm::new(None, "Team sync".into());
        form.event = true;
        form.location = "Meeting room 2".into();
        assert_eq!(form.build().unwrap().location, "Meeting room 2");

        form.event = false;

        let plain = form.build().expect("a valid form");
        assert!(!plain.event);
        assert!(
            plain.location.is_empty(),
            "a reminder that is no longer an event keeps no location"
        );
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
