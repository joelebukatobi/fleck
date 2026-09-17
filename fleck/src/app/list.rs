//! The notes list window: search bar, cards, and their rename state.

use std::time::{Duration, SystemTime};

use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;
use fleck_core::{display_name, Note};
use uuid::Uuid;

use super::style::{
    card_button_class, card_container_style, card_heading_style, icon_button_class,
    search_input_style, IconHoverRole,
};
use super::{Fleck, Message, DEFAULT_WINDOW_SIZE};

/// How many lines of body text a card previews.
pub(super) const PREVIEW_LINES: usize = 2;

/// Whether a note matches the notes-list search bar's query: a
/// case-insensitive substring match against either its display name or its
/// full body text. An empty query matches everything.
pub(super) fn matches_search(query: &str, name: &str, body: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    name.to_lowercase().contains(&query) || body.to_lowercase().contains(&query)
}

/// The first `max_lines` non-empty lines of a note's body to show as a
/// card's preview. If the note has no explicit name, `display_name` falls
/// back to showing the first non-empty line as the name (see
/// `fleck_core::display_name`/`title`) - so that line is skipped
/// here to avoid repeating it in the preview.
pub(super) fn preview_lines(note: &Note, max_lines: usize) -> Vec<String> {
    let lines: Vec<&str> = note.body.lines().collect();
    let start = if note.frontmatter.name.trim().is_empty() {
        // Skip past (and including) the first non-empty line - the one
        // `display_name` is already showing as the card's title.
        match lines.iter().position(|l| !l.trim().is_empty()) {
            Some(i) => i + 1,
            None => lines.len(),
        }
    } else {
        0
    };
    lines
        .get(start..)
        .unwrap_or(&[])
        .iter()
        .filter(|l| !l.trim().is_empty())
        .take(max_lines)
        .map(|l| l.trim().to_string())
        .collect()
}

pub(super) const MINUTE: Duration = Duration::from_secs(60);
pub(super) const HOUR: Duration = Duration::from_hours(1);
pub(super) const DAY: Duration = Duration::from_hours(24);
pub(super) const WEEK: Duration = Duration::from_hours(168);

/// Formats how long ago a note was last edited, from the elapsed time since
/// its file's mtime. No date/time crate: relative phrasing needs no
/// timezone handling, just bucketed arithmetic on a `Duration`. Wording and
/// plurals come from the translation files.
pub(super) fn relative_time(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    // Plain locals: `fl!` takes each argument as `name = variable`.
    let (minutes, hours, days, weeks) = (secs / 60, secs / 3600, secs / 86_400, secs / 604_800);
    if elapsed < MINUTE {
        crate::fl!("just-now")
    } else if elapsed < HOUR {
        crate::fl!("minutes-ago", count = minutes)
    } else if elapsed < DAY {
        crate::fl!("hours-ago", count = hours)
    } else if elapsed < WEEK {
        crate::fl!("days-ago", count = days)
    } else {
        crate::fl!("weeks-ago", count = weeks)
    }
}

/// Layout constants for the notes list panel (`view_list`/`view_card`) - one
/// place for the design pass to change spacing and corner radius.
pub(super) const LIST_PANEL_PADDING: u16 = 8;
pub(super) const SEARCH_PADDING_X: u16 = 8;
/// Inset around the search bar's magnifier (top, left, bottom). The right
/// side is 0 so the icon lines up with the card trash icons.
pub(super) const SEARCH_ICON_INSET: u16 = 8;
/// Padding of the card's content section (preview lines, then time) -
/// repurposed from the old single-container card padding now that the card
/// has two padded sections instead of one.
pub(super) const CARD_PADDING: u16 = 8;
/// Padding inside the card's heading strip (title + rename pencil).
pub(super) const CARD_HEADING_PADDING_Y: u16 = 4;
pub(super) const CARD_HEADING_PADDING_X: u16 = 8;
/// Gap between the heading strip and the content section below it.
pub(super) const CARD_HEADING_GAP: u16 = 4;
/// Gap between a card's title and its rename pencil, within the heading row.
pub(super) const CARD_HEADING_ROW_SPACING: u16 = 8;
/// Gap between the pencil and trash buttons. 0: their facing paddings
/// (`CARD_ACTION_INNER_PADDING`) set the distance between the icons.
pub(super) const CARD_ACTION_SPACING: u16 = 0;
/// Padding on the outer sides of the rename and delete buttons (top, bottom,
/// and the side away from the other button). Matches libcosmic's own
/// `extra_small` icon-button padding, so the delete icon stays 8 px from the
/// card's right edge.
pub(super) const CARD_ACTION_PADDING: u16 = 8;
/// Padding on the side where the rename and delete buttons face each other.
/// 4 + 4 puts their icons 8 px apart.
pub(super) const CARD_ACTION_INNER_PADDING: u16 = 4;
/// Right padding of the card heading. 0 so the trash icon, inside its own
/// 8 px button padding, sits 8 px from the card's right edge.
pub(super) const CARD_HEADING_PADDING_RIGHT: u16 = 0;
/// Gap between the preview lines and the timestamp in a card's content.
pub(super) const CARD_CONTENT_SPACING: u16 = 4;
pub(super) const CARD_SPACING: u16 = 8;
/// Dialogs over the notes list are this fraction of the list window's width.
/// libcosmic's own default is a fixed 570 px, wider than the list window.
pub(super) const DIALOG_WIDTH_FRACTION: f32 = 0.75;

/// Width for a dialog over a window `window_width` wide.
pub(super) fn dialog_width(window_width: f32) -> f32 {
    if window_width.is_finite() && window_width > 0.0 {
        window_width * DIALOG_WIDTH_FRACTION
    } else {
        DEFAULT_WINDOW_SIZE.0 as f32 * DIALOG_WIDTH_FRACTION
    }
}

/// Minimum gap between a card's right edge and the scrollbar's widest
/// extent, embedded via `Scrollbar::spacing` (see `view_list`) so it only
/// takes up space while a scrollbar is actually shown.
pub(super) const SCROLLBAR_GAP: f32 = 4.0;
/// Inset at the top and bottom of the card list's scrollbar track. libcosmic's
/// `widget::scrollable` defaults this to 8 px; 0 lets the thumb run the full
/// height of the list.
pub(super) const SCROLLBAR_PADDING: f32 = 0.0;
/// The list scrollbar's rail thickness - reserved layout space, constant
/// regardless of hover, so the cards never shift width when the scroller
/// widens (see `SCROLLBAR_SCROLLER_WIDTH_HOVER`).
pub(super) const SCROLLBAR_WIDTH: f32 = 8.0;
/// The scroller's thickness at rest ("slim"): thinner than
/// `SCROLLBAR_WIDTH`, centered within the (unchanging) rail lane.
pub(super) const SCROLLBAR_SCROLLER_WIDTH_REST: f32 = 4.0;
/// The scroller's thickness while the list is hovered ("wide"): matches
/// `SCROLLBAR_WIDTH`, filling the rail lane. Changing only `scroller_width`
/// between the two states is what keeps this from ever moving the cards -
/// `Scrollable::layout`'s reserved padding is computed from `width` and
/// `margin`, not `scroller_width` (see `iced/widget/src/scrollable.rs`
/// ~line 616 in the pinned iced fork).
pub(super) const SCROLLBAR_SCROLLER_WIDTH_HOVER: f32 = 8.0;

/// Whether the notes list's rename control is currently editing `uuid`'s
/// card - the pure question a card's view asks to pick between its display
/// and rename widget tree.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) enum RenameState {
    #[default]
    Idle,
    Editing {
        uuid: Uuid,
        text: String,
    },
}

impl RenameState {
    pub(super) fn start(uuid: Uuid, current_name: &str) -> Self {
        RenameState::Editing {
            uuid,
            text: current_name.to_string(),
        }
    }

    pub(super) fn is_editing(&self, uuid: Uuid) -> bool {
        matches!(self, RenameState::Editing { uuid: u, .. } if *u == uuid)
    }

    pub(super) fn text(&self) -> &str {
        match self {
            RenameState::Editing { text, .. } => text,
            RenameState::Idle => "",
        }
    }

    /// Updates the in-progress text, a no-op if nothing is being edited.
    pub(super) fn with_input(self, new_text: String) -> Self {
        match self {
            RenameState::Editing { uuid, .. } => RenameState::Editing {
                uuid,
                text: new_text,
            },
            RenameState::Idle => RenameState::Idle,
        }
    }
}

impl Fleck {
    /// A note's cached mtime, or `UNIX_EPOCH` for one never observed on
    /// disk yet (defensive only - every note in `self.notes` is either
    /// loaded from disk at `init` or written by `create_note`, both of
    /// which populate `mtimes`).
    pub(super) fn note_mtime(&self, uuid: Uuid) -> SystemTime {
        self.mtimes
            .get(&uuid)
            .copied()
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }

    /// One note's card: its name (or, in rename mode, a text input in its
    /// place) with rename and delete controls beside it, a couple of lines
    /// of body preview, and a relative last-edited time. Clicking anywhere
    /// but those controls opens the note - each is a nested `button` inside
    /// the card's own button, so iced's event dispatch hands the press to
    /// whichever is deepest (the control) first and never lets it also
    /// reach the card's own `on_press`, same as the existing rename pencil.
    pub(super) fn view_card(&self, note: &Note) -> Element<'_, Message> {
        let uuid = note.frontmatter.uuid;
        let editing = self.rename.is_editing(uuid);

        // The title swaps for a text input in rename mode, but stays in the
        // same slot of the same heading row - the pencil button next to it
        // never moves or disappears, so the row's shape never changes.
        let title: Element<'_, Message> = if editing {
            widget::text_input("", self.rename.text())
                .id(self.rename_input_id.clone())
                .on_input(Message::RenameInput)
                .on_submit(|_| Message::RenameSave)
                .on_unfocus(Message::RenameCancel)
                .width(Length::Fill)
                .into()
        } else {
            widget::text::heading(display_name(note).to_string())
                .width(Length::Fill)
                .into()
        };

        let heading_row = widget::Row::with_capacity(2)
            .spacing(CARD_HEADING_ROW_SPACING)
            .align_y(Alignment::Center)
            .push(title)
            .push(
                widget::Row::with_capacity(2)
                    .spacing(CARD_ACTION_SPACING)
                    .align_y(Alignment::Center)
                    .push(
                        widget::button::icon(crate::icons::edit_pencil())
                            .extra_small()
                            .padding([
                                CARD_ACTION_PADDING,
                                CARD_ACTION_INNER_PADDING,
                                CARD_ACTION_PADDING,
                                CARD_ACTION_PADDING,
                            ])
                            .on_press(Message::RenameStart(uuid))
                            .class(icon_button_class(IconHoverRole::Accent)),
                    )
                    .push(
                        widget::button::icon(crate::icons::trash())
                            .extra_small()
                            .padding([
                                CARD_ACTION_PADDING,
                                CARD_ACTION_PADDING,
                                CARD_ACTION_PADDING,
                                CARD_ACTION_INNER_PADDING,
                            ])
                            .on_press(Message::DeleteStart(uuid))
                            .class(icon_button_class(IconHoverRole::Destructive)),
                    ),
            );

        let heading = widget::container(heading_row)
            .class(cosmic::theme::Container::custom(card_heading_style))
            .padding([
                CARD_HEADING_PADDING_Y,
                CARD_HEADING_PADDING_RIGHT,
                CARD_HEADING_PADDING_Y,
                CARD_HEADING_PADDING_X,
            ])
            .width(Length::Fill);

        let mut content_col =
            widget::Column::with_capacity(1 + PREVIEW_LINES).spacing(CARD_CONTENT_SPACING);
        for line in preview_lines(note, PREVIEW_LINES) {
            content_col = content_col.push(widget::text::body(line));
        }
        let elapsed = SystemTime::now()
            .duration_since(self.note_mtime(uuid))
            .unwrap_or(Duration::ZERO);
        content_col = content_col.push(widget::text::caption(relative_time(elapsed)));

        let content = widget::container(content_col)
            .padding(CARD_PADDING)
            .width(Length::Fill);

        let card_body = widget::Column::with_capacity(2)
            .spacing(CARD_HEADING_GAP)
            .push(heading)
            .push(content);

        // Bare (no background) in both branches, so rename mode and the
        // button-wrapped display mode size and position identically - only
        // one of them actually paints a card surface. In display mode the
        // button itself is that surface (see `card_button_class`); in
        // rename mode, with no button to paint it, this container carries
        // the same resting-card style directly so the card still reads as
        // a card while its name is being edited.
        let card = widget::container(card_body).width(Length::Fill);

        if editing {
            // Mid-rename, the card isn't a pick target - the text input
            // already owns clicks/focus here.
            card.class(cosmic::theme::Container::custom(card_container_style))
                .into()
        } else {
            // No padding: libcosmic buttons default to 5 px, which inset the card from
            // the search bar and drew the hover state 5 px outside the card.
            widget::button::custom(card)
                .padding(0)
                .on_press(Message::PickNote(uuid))
                .class(card_button_class())
                .width(Length::Fill)
                .into()
        }
    }

    /// The notes list: a search bar filtering the cards below it (most
    /// recently edited first), each showing a note's name, a body preview,
    /// and when it was last edited. The restore prompt is a separate modal
    /// dialog (`Application::dialog`), not part of this view.
    pub(super) fn view_list(&self) -> Element<'_, Message> {
        let mut notes: Vec<&Note> = self
            .notes
            .values()
            .filter(|n| matches_search(&self.search, display_name(n), &n.body))
            .collect();
        notes.sort_by(|a, b| {
            self.note_mtime(b.frontmatter.uuid)
                .cmp(&self.note_mtime(a.frontmatter.uuid))
        });

        // `text_input`, not `search_input`: the latter always adds COSMIC's
        // magnifier on the left, and the placeholder should start right at
        // the bar's padding, in line with the card titles.
        let search = widget::text_input(crate::fl!("search-notes"), self.search.clone())
            .id(self.search_input_id.clone())
            .on_input(Message::SearchChanged)
            // The magnifier sits at the right edge, where a clear button
            // would usually go; there is no clear button.
            .trailing_icon(
                widget::icon(crate::icons::search())
                    .size(16)
                    .apply(widget::container)
                    // No right inset, so it sits 8 px from the bar's right
                    // edge, in line with the card trash icons.
                    .padding([SEARCH_ICON_INSET, 0, SEARCH_ICON_INSET, SEARCH_ICON_INSET])
                    .into(),
            )
            .padding([0, SEARCH_PADDING_X])
            .style(cosmic::theme::TextInput::Custom {
                active: search_input_style(
                    <cosmic::Theme as cosmic::widget::text_input::StyleSheet>::active,
                ),
                error: search_input_style(
                    <cosmic::Theme as cosmic::widget::text_input::StyleSheet>::error,
                ),
                hovered: search_input_style(
                    <cosmic::Theme as cosmic::widget::text_input::StyleSheet>::hovered,
                ),
                focused: search_input_style(
                    <cosmic::Theme as cosmic::widget::text_input::StyleSheet>::focused,
                ),
                disabled: search_input_style(
                    <cosmic::Theme as cosmic::widget::text_input::StyleSheet>::disabled,
                ),
            });

        let mut cards = widget::Column::with_capacity(notes.len()).spacing(CARD_SPACING);
        for note in notes {
            cards = cards.push(self.view_card(note));
        }

        // Slim until hovered, then wider, like other COSMIC apps: the
        // reserved rail lane (`scrollbar_width`) never changes, so the
        // cards never shift - only the scroller's own thickness
        // (`scroller_width`) does, between `SCROLLBAR_SCROLLER_WIDTH_REST`
        // and `SCROLLBAR_SCROLLER_WIDTH_HOVER`, tracked via
        // `Message::ListScrollHover` since neither is reachable through
        // libcosmic's `Scrollable` style catalog alone (its `Style` only
        // ever carries colour/border, never a size - see
        // `card_button_class`'s sibling investigation in `docs`/the phase
        // report). `spacing(SCROLLBAR_GAP)` embeds the scrollbar - the
        // cards only lose that width when a scrollbar is actually shown
        // (`Scrollable::layout`), so a short list still runs full width.
        let scroller_width = if self.list_scroll_hovered {
            SCROLLBAR_SCROLLER_WIDTH_HOVER
        } else {
            SCROLLBAR_SCROLLER_WIDTH_REST
        };
        let list_scrollable = widget::scrollable(cards)
            .width(Length::Fill)
            .height(Length::Fill)
            .spacing(SCROLLBAR_GAP)
            .scrollbar_width(SCROLLBAR_WIDTH)
            .scroller_width(scroller_width)
            .scrollbar_padding(SCROLLBAR_PADDING)
            // `Minimal` leaves the track transparent, so only the thumb shows;
            // `Permanent` (the default) paints the track at its full 8 px.
            .class(cosmic::style::iced::Scrollable::Minimal);
        let list_scrollable = widget::mouse_area(list_scrollable)
            .on_enter(Message::ListScrollHover(true))
            .on_exit(Message::ListScrollHover(false));

        let content = widget::Column::with_capacity(2)
            .spacing(8)
            .push(search)
            .push(list_scrollable);

        widget::container(content)
            .class(cosmic::theme::Container::WindowBackground)
            .padding(LIST_PANEL_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact pixel values are the point of these tests
mod tests {
    use super::*;

    /// `relative_time` without Fluent's bidi isolation marks around numbers,
    /// so the English wording can be compared directly.
    fn relative_time(elapsed: Duration) -> String {
        super::relative_time(elapsed).replace(['\u{2068}', '\u{2069}'], "")
    }
    use fleck_core::{Frontmatter, FORMAT_VERSION};

    #[test]
    fn dialog_width_is_three_quarters_of_the_window() {
        assert_eq!(dialog_width(512.0), 384.0);
        assert_eq!(dialog_width(1000.0), 750.0);
    }

    #[test]
    fn dialog_width_falls_back_for_nonsense_widths() {
        let fallback = DEFAULT_WINDOW_SIZE.0 as f32 * DIALOG_WIDTH_FRACTION;
        assert_eq!(dialog_width(0.0), fallback);
        assert_eq!(dialog_width(-5.0), fallback);
        assert_eq!(dialog_width(f32::NAN), fallback);
        assert_eq!(dialog_width(f32::INFINITY), fallback);
    }

    #[test]
    fn empty_search_matches_everything() {
        assert!(matches_search("", "Shopping List", "milk\neggs\n"));
        assert!(matches_search("", "", ""));
    }

    #[test]
    fn search_matches_the_name_case_insensitively() {
        assert!(matches_search("shop", "Shopping List", "milk\n"));
        assert!(matches_search("SHOPPING", "Shopping List", "milk\n"));
    }

    #[test]
    fn search_matches_the_body_case_insensitively() {
        assert!(matches_search("milk", "Groceries", "buy Milk and eggs\n"));
    }

    #[test]
    fn search_with_no_match_in_name_or_body_excludes_the_note() {
        assert!(!matches_search("pizza", "Groceries", "milk\neggs\n"));
    }

    fn note_named(name: &str, body: &str) -> Note {
        Note {
            frontmatter: Frontmatter {
                version: FORMAT_VERSION,
                uuid: Uuid::new_v4(),
                created: "2026-09-04T10:15:00Z".to_string(),
                color: "yellow".to_string(),
                name: name.to_string(),
            },
            body: body.to_string(),
        }
    }

    #[test]
    fn preview_skips_the_title_line_for_an_unnamed_note() {
        let note = note_named("", "Groceries\nmilk\neggs\nbread\n");
        // "Groceries" is what display_name already shows as the title -
        // must not be repeated as the first preview line.
        assert_eq!(
            preview_lines(&note, 2),
            vec!["milk".to_string(), "eggs".to_string()]
        );
    }

    #[test]
    fn preview_keeps_the_first_line_for_a_named_note() {
        let note = note_named("Shopping List", "Groceries\nmilk\neggs\n");
        assert_eq!(
            preview_lines(&note, 2),
            vec!["Groceries".to_string(), "milk".to_string()]
        );
    }

    #[test]
    fn preview_skips_blank_lines() {
        let note = note_named("", "Groceries\n\n\nmilk\neggs\n");
        assert_eq!(
            preview_lines(&note, 2),
            vec!["milk".to_string(), "eggs".to_string()]
        );
    }

    #[test]
    fn preview_is_empty_for_an_unnamed_note_with_only_a_title_line() {
        let note = note_named("", "Groceries\n");
        assert!(preview_lines(&note, 2).is_empty());
    }

    #[test]
    fn preview_is_empty_for_a_totally_empty_note() {
        let note = note_named("", "");
        assert!(preview_lines(&note, 2).is_empty());
    }

    #[test]
    fn relative_time_just_now_below_a_minute() {
        assert_eq!(relative_time(Duration::from_secs(0)), "just now");
        assert_eq!(relative_time(Duration::from_secs(59)), "just now");
    }

    #[test]
    fn relative_time_minutes_boundary() {
        assert_eq!(relative_time(Duration::from_secs(60)), "1 min ago");
        assert_eq!(relative_time(Duration::from_mins(4)), "4 min ago");
        assert_eq!(
            relative_time(Duration::from_secs(59 * 60 + 59)),
            "59 min ago"
        );
    }

    #[test]
    fn relative_time_hours_boundary() {
        assert_eq!(relative_time(Duration::from_hours(1)), "1 hour ago");
        assert_eq!(relative_time(Duration::from_hours(2)), "2 hours ago");
        assert_eq!(
            relative_time(Duration::from_secs(23 * 60 * 60 + 3599)),
            "23 hours ago"
        );
    }

    #[test]
    fn relative_time_days_boundary() {
        assert_eq!(relative_time(Duration::from_hours(24)), "1 day ago");
        assert_eq!(relative_time(Duration::from_hours(72)), "3 days ago");
        assert_eq!(
            relative_time(Duration::from_secs(6 * 24 * 60 * 60 + 86399)),
            "6 days ago"
        );
    }

    #[test]
    fn relative_time_weeks_boundary() {
        assert_eq!(relative_time(Duration::from_hours(168)), "1 week ago");
        assert_eq!(relative_time(Duration::from_hours(504)), "3 weeks ago");
    }

    #[test]
    fn rename_state_starts_idle() {
        assert_eq!(RenameState::default(), RenameState::Idle);
    }

    #[test]
    fn rename_state_start_enters_editing_with_the_current_name() {
        let uuid = Uuid::new_v4();
        let state = RenameState::start(uuid, "Old name");
        assert!(state.is_editing(uuid));
        assert_eq!(state.text(), "Old name");
    }

    #[test]
    fn rename_state_only_the_started_note_is_editing() {
        let uuid = Uuid::new_v4();
        let other = Uuid::new_v4();
        let state = RenameState::start(uuid, "");
        assert!(
            !state.is_editing(other),
            "only one card may be in rename mode at a time"
        );
    }

    #[test]
    fn rename_state_with_input_updates_text_while_editing() {
        let uuid = Uuid::new_v4();
        let state = RenameState::start(uuid, "").with_input("New name".to_string());
        assert_eq!(state.text(), "New name");
        assert!(state.is_editing(uuid));
    }

    #[test]
    fn rename_state_with_input_is_a_no_op_when_idle() {
        let state = RenameState::Idle.with_input("typed while idle".to_string());
        assert_eq!(state, RenameState::Idle);
    }
}
