//! Reminders: their own items, stored together in one file, each with its own
//! text and time. A reminder may point at a note, in which case firing it can
//! open that note, but it doesn't have to.

use chrono::{DateTime, Datelike, Days, Local, Months, NaiveTime, TimeZone, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

/// How often a reminder comes back after it fires.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    /// Fires once, then it is done.
    #[default]
    Once,
    Daily,
    /// Monday to Friday.
    Weekdays,
    /// The same day each week.
    Weekly,
    /// The same day each month. A reminder set for the 31st falls back to the
    /// last day of a shorter month.
    Monthly,
}

impl Repeat {
    pub const ALL: [Repeat; 5] = [
        Repeat::Once,
        Repeat::Daily,
        Repeat::Weekdays,
        Repeat::Weekly,
        Repeat::Monthly,
    ];

    /// When this reminder comes back after firing at `fired`, or `None` for a
    /// one-off. Times of day are kept, so a daily 9am stays 9am even across a
    /// daylight-saving change.
    #[must_use]
    pub fn next_after(self, fired: DateTime<Local>) -> Option<DateTime<Local>> {
        let next = match self {
            Repeat::Once => return None,
            Repeat::Daily => fired.checked_add_days(Days::new(1))?,
            Repeat::Weekdays => {
                let days = match fired.weekday() {
                    Weekday::Fri => 3,
                    Weekday::Sat => 2,
                    _ => 1,
                };
                fired.checked_add_days(Days::new(days))?
            }
            Repeat::Weekly => fired.checked_add_days(Days::new(7))?,
            Repeat::Monthly => fired.checked_add_months(Months::new(1))?,
        };
        Some(next)
    }
}

/// One reminder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reminder {
    pub uuid: Uuid,
    /// What to remind about. Empty when it only points at a note, in which
    /// case the app shows that note's name instead.
    #[serde(default)]
    pub text: String,
    /// When it is next due, as an RFC 3339 timestamp.
    pub due: String,
    #[serde(default)]
    pub repeat: Repeat,
    /// The note this reminder is about, if any.
    #[serde(default)]
    pub note: Option<Uuid>,
}

impl Reminder {
    /// A new reminder due at `due`.
    #[must_use]
    pub fn new(text: String, due: DateTime<Local>, repeat: Repeat, note: Option<Uuid>) -> Self {
        Self {
            uuid: Uuid::new_v4(),
            text,
            due: format_due(due),
            repeat,
            note,
        }
    }

    /// When it is due, in local time. `None` for a timestamp that can't be
    /// read - a hand-edited file, say - which the app treats as not due.
    #[must_use]
    pub fn due_at(&self) -> Option<DateTime<Local>> {
        parse_due(&self.due)
    }

    /// Whether this reminder should fire at `now`.
    #[must_use]
    pub fn is_due(&self, now: DateTime<Local>) -> bool {
        self.due_at().is_some_and(|due| due <= now)
    }

    /// The same reminder moved to its next occurrence, or `None` when it was a
    /// one-off and is now finished.
    #[must_use]
    pub fn after_firing(&self) -> Option<Self> {
        let fired = self.due_at()?;
        let mut next = self.repeat.next_after(fired)?;
        // A reminder that was missed - Fleck was closed for a week, say -
        // moves forward to the next occurrence still ahead, not through every
        // one it slept through.
        let now = Local::now();
        while next <= now {
            next = self.repeat.next_after(next)?;
        }
        Some(Self {
            due: format_due(next),
            ..self.clone()
        })
    }
}

fn format_due(due: DateTime<Local>) -> String {
    due.with_timezone(&Utc).to_rfc3339()
}

fn parse_due(due: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(due)
        .ok()
        .map(|due| due.with_timezone(&Local))
}

/// A local date and time built from the parts a dialog collects. `None` for a
/// date that doesn't exist, or a time that a daylight-saving jump skips.
#[must_use]
pub fn local_at(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
) -> Option<DateTime<Local>> {
    let date = chrono::NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    Local.from_local_datetime(&date.and_time(time)).earliest()
}

/// Every reminder, kept together in one file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Reminders {
    #[serde(default)]
    pub reminders: Vec<Reminder>,
}

impl Reminders {
    /// Reads the reminders file. A missing or unreadable file yields none,
    /// like window state: a broken file must not stop Fleck starting.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the file atomically, through a temporary file beside it.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string(self).expect("reminders are serializable");
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, path)
    }

    /// The reminders due at `now`, soonest first.
    #[must_use]
    pub fn due(&self, now: DateTime<Local>) -> Vec<&Reminder> {
        let mut due: Vec<&Reminder> = self.reminders.iter().filter(|r| r.is_due(now)).collect();
        due.sort_by_key(|r| r.due.clone());
        due
    }

    /// Replaces a fired reminder with its next occurrence, or drops it when it
    /// was a one-off.
    pub fn reschedule(&mut self, uuid: Uuid) {
        let Some(index) = self.reminders.iter().position(|r| r.uuid == uuid) else {
            return;
        };
        match self.reminders[index].after_firing() {
            Some(next) => self.reminders[index] = next,
            None => {
                self.reminders.remove(index);
            }
        }
    }

    /// Adds a reminder, or replaces one with the same id.
    pub fn insert(&mut self, reminder: Reminder) {
        match self.reminders.iter().position(|r| r.uuid == reminder.uuid) {
            Some(index) => self.reminders[index] = reminder,
            None => self.reminders.push(reminder),
        }
    }

    pub fn remove(&mut self, uuid: Uuid) {
        self.reminders.retain(|r| r.uuid != uuid);
    }

    /// Reminders in the order they should be listed: soonest first.
    #[must_use]
    pub fn sorted(&self) -> Vec<&Reminder> {
        let mut all: Vec<&Reminder> = self.reminders.iter().collect();
        all.sort_by_key(|r| r.due.clone());
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Local> {
        local_at(year, month, day, hour, minute).expect("a real local time")
    }

    #[test]
    fn a_one_off_reminder_is_finished_after_it_fires() {
        let reminder = Reminder::new(
            "post the letter".into(),
            at(2026, 10, 1, 9, 0),
            Repeat::Once,
            None,
        );
        assert_eq!(reminder.after_firing(), None);
    }

    #[test]
    fn repeats_move_to_the_next_occurrence() {
        // A Thursday.
        let thursday = at(2026, 10, 1, 9, 0);
        assert_eq!(
            Repeat::Daily.next_after(thursday),
            Some(at(2026, 10, 2, 9, 0))
        );
        assert_eq!(
            Repeat::Weekly.next_after(thursday),
            Some(at(2026, 10, 8, 9, 0))
        );
        assert_eq!(
            Repeat::Monthly.next_after(thursday),
            Some(at(2026, 11, 1, 9, 0))
        );
        assert_eq!(Repeat::Once.next_after(thursday), None);
    }

    #[test]
    fn weekdays_skip_the_weekend() {
        let friday = at(2026, 10, 2, 9, 0);
        assert_eq!(
            Repeat::Weekdays.next_after(friday),
            Some(at(2026, 10, 5, 9, 0)),
            "Friday's next weekday is Monday"
        );
        let saturday = at(2026, 10, 3, 9, 0);
        assert_eq!(
            Repeat::Weekdays.next_after(saturday),
            Some(at(2026, 10, 5, 9, 0))
        );
    }

    #[test]
    fn a_monthly_reminder_on_the_31st_falls_back_to_the_months_last_day() {
        let january = at(2026, 1, 31, 9, 0);
        assert_eq!(
            Repeat::Monthly.next_after(january),
            Some(at(2026, 2, 28, 9, 0))
        );
    }

    #[test]
    fn a_missed_repeat_moves_to_the_next_occurrence_ahead_not_every_one_it_slept_through() {
        let long_ago = Local::now() - chrono::Duration::days(10);
        let reminder = Reminder::new("water the plants".into(), long_ago, Repeat::Daily, None);

        let next = reminder.after_firing().expect("a daily reminder repeats");

        let due = next.due_at().expect("a readable time");
        assert!(due > Local::now(), "the next occurrence must be ahead");
        assert!(
            due <= Local::now() + chrono::Duration::days(1),
            "and within a day, not ten firings later"
        );
    }

    #[test]
    fn due_lists_what_has_come_round_soonest_first() {
        let now = Local::now();
        let mut reminders = Reminders::default();
        let later = Reminder::new(
            "later".into(),
            now + chrono::Duration::hours(1),
            Repeat::Once,
            None,
        );
        let overdue = Reminder::new(
            "overdue".into(),
            now - chrono::Duration::hours(2),
            Repeat::Once,
            None,
        );
        let just_now = Reminder::new(
            "just now".into(),
            now - chrono::Duration::minutes(1),
            Repeat::Once,
            None,
        );
        reminders.insert(later);
        reminders.insert(overdue.clone());
        reminders.insert(just_now.clone());

        let due = reminders.due(now);

        assert_eq!(due.len(), 2, "the one an hour away is not due");
        assert_eq!(due[0].uuid, overdue.uuid, "the oldest comes first");
        assert_eq!(due[1].uuid, just_now.uuid);
    }

    #[test]
    fn rescheduling_repeats_and_drops_one_offs() {
        let now = Local::now();
        let mut reminders = Reminders::default();
        let daily = Reminder::new(
            "daily".into(),
            now - chrono::Duration::minutes(5),
            Repeat::Daily,
            None,
        );
        let once = Reminder::new(
            "once".into(),
            now - chrono::Duration::minutes(5),
            Repeat::Once,
            None,
        );
        reminders.insert(daily.clone());
        reminders.insert(once.clone());

        reminders.reschedule(daily.uuid);
        reminders.reschedule(once.uuid);

        assert_eq!(reminders.reminders.len(), 1, "the one-off is gone");
        let moved = &reminders.reminders[0];
        assert_eq!(moved.uuid, daily.uuid);
        assert!(moved.due_at().unwrap() > now, "the daily one moved ahead");
    }

    #[test]
    fn reminders_round_trip_through_their_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reminders.toml");
        let mut reminders = Reminders::default();
        reminders.insert(Reminder::new(
            "call the dentist".into(),
            at(2026, 10, 1, 9, 30),
            Repeat::Weekdays,
            Some(Uuid::from_u128(5)),
        ));

        reminders.save(&path).unwrap();

        assert_eq!(Reminders::load(&path), reminders);
    }

    #[test]
    fn a_missing_or_broken_file_yields_no_reminders() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Reminders::load(&dir.path().join("gone.toml")),
            Reminders::default()
        );
        let broken = dir.path().join("broken.toml");
        std::fs::write(&broken, "this is not toml {{{").unwrap();
        assert_eq!(Reminders::load(&broken), Reminders::default());
    }

    #[test]
    fn an_unreadable_time_is_never_due() {
        let reminder = Reminder {
            uuid: Uuid::from_u128(1),
            text: "hand-edited".into(),
            due: "not a timestamp".into(),
            repeat: Repeat::Once,
            note: None,
        };
        assert!(!reminder.is_due(Local::now()));
        assert_eq!(reminder.due_at(), None);
    }
}
