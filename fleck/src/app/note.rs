//! A note window: the body editor over its ruled-paper canvas.

use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::iced::core::text::LineHeight;
use cosmic::iced::widget::Stack;
use cosmic::iced::{window, Border, Color, Length, Pixels, Size};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::text_editor;

use fleck_core::{display_name, Note};
use uuid::Uuid;

use super::list::dialog_width;
use super::style::{
    card_button_class, icon_button_class, menu_item_button_class, note_paper_style,
    note_title_bar_style, rgb, swatch_style, text_button_class, IconHoverRole,
};
use super::theme::AppTheme;
use super::{images, DictationState, Fleck, Message, NoteDialog, DEFAULT_WINDOW_SIZE};
use crate::palette::Colour;
use crate::ruled::RuledLines;
use crate::undo::EditKind;

/// The strip's scrollable, named per window so the wheel can drive it: the
/// mouse_area around the strip reports the wheel, and the app scrolls the
/// strip to the offset it works out (see `Message::NoteStripWheel`).
pub(super) fn strip_id(window: window::Id) -> cosmic::widget::Id {
    cosmic::widget::Id::new(format!("fleck-strip-{window:?}"))
}

/// How wide a tab is in the strip across the top of a note: enough of a name
/// to tell one from another, and four of them across a note's default width.
const STUB_WIDTH: f32 = 110.0;
/// How tall a tab is once the strip is awake.
const STUB_HEIGHT_AWAKE: f32 = 28.0;
/// How tall a tab is at rest, before the mouse has been near it: a bar of the
/// note's colour and nothing else, so the strip reads as tabs tucked under the
/// title bar rather than a panel over the note.
const STUB_REST_HEIGHT: f32 = 6.0;
/// How tall a tab's panel is when the mouse is on it: the name and the first
/// lines of the note.
const STUB_PANEL_HEIGHT: f32 = 76.0;
/// The gap between the two lines of the panel a hovered stub opens into. The
/// stubs themselves have no gap: they sit flush, divided by a line each paints
/// along its own bottom edge (see `stub::Stub`).
const STUB_PANEL_GAP: u16 = 4;
/// The hovered stub slides out to this width, showing the note's whole name
/// and the start of its text, written the usual way round.
const STUB_PANEL_WIDTH: f32 = 240.0;
/// Padding inside the panel a hovered stub opens into. A resting stub has
/// none: its title is painted to fill the whole stub.
const STUB_PADDING: u16 = 8;
/// How much of the hovered note's text the panel shows.
const STUB_PANEL_LINES: usize = 2;
/// How tall the strip's reach is while it sleeps: a band along the top of the
/// note, wider than the 6 px bars so the strip is easy to find, but nothing
/// like the whole note - the mouse has to leave this band for the strip to go
/// back to sleep.
const STUB_BAND: f32 = 18.0;

/// How far one notch of the wheel moves the strip.
const STUB_WHEEL_LINE: f32 = 32.0;

/// The size of a stub's title.
const STUB_TEXT_SIZE: f32 = 13.0;

/// Size of the note body's text, in logical pixels.
pub(super) const BODY_TEXT_SIZE: f32 = 14.0;

/// Height of one line of the note body, in logical pixels. Set explicitly
/// (rather than left to font metrics) and shared, unchanged, with
/// `ruled::RuledLines::line_height` so the dotted rules drawn behind the
/// body can never drift out of alignment with the text sitting on them.
pub(super) const BODY_LINE_HEIGHT: f32 = 22.0;

/// The body `text_editor`'s padding, in logical pixels, above and below the
/// text (none at the sides).
/// Shared with `ruled::RuledLines::padding_top` for the same reason as
/// `BODY_LINE_HEIGHT` - the first rule's offset has to account for exactly
/// this much space above the first line of text.
pub(super) const BODY_PADDING: f32 = 8.0;

/// The note menu: its width, its inner padding, and each row's padding.
const MENU_WIDTH: f32 = 200.0;
/// The "Settings" text button: COSMIC's default text-button height, and its
/// side padding. The label sits 18 px from the window's left edge, about as
/// far as the visible close cross sits from the right; the menu opens just
/// below the button, in line with the left edge of its hover background.
const MENU_BUTTON_HEIGHT: u16 = 32;
const MENU_BUTTON_PADDING_X: u16 = 11;
/// libcosmic's header bar padding on each side (Standard density, not
/// maximised). Fixed inside libcosmic; mirrored here only for alignment.
const HEADER_BAR_PADDING_X: u16 = 7;
/// The lined area's side padding: in line with the "Settings" label and the
/// menu under it on the left, and the same distance in from the right.
const NOTE_PADDING_X: u16 = HEADER_BAR_PADDING_X + MENU_BUTTON_PADDING_X;
const NOTE_PADDING_Y: u16 = 12;
const MENU_PADDING: u16 = 4;
/// The colour dialog's swatch grid.
const SWATCHES_PER_ROW: usize = 3;
const SWATCH_SIZE: f32 = 32.0;
const SWATCH_GAP: u16 = 8;
const SWATCH_LABEL_GAP: u16 = 4;
const SWATCH_PADDING: u16 = 8;
/// The thumbnail row under a note's text.
const THUMBNAIL_HEIGHT: f32 = 64.0;
const THUMBNAIL_GAP: u16 = 8;
const THUMBNAIL_PADDING: u16 = 4;
/// The tallest a picture is drawn in the image dialog.
const IMAGE_DIALOG_MAX_HEIGHT: f32 = 320.0;
/// The dictation button: its icon, the padding that makes it round, and how
/// far it floats from the note's bottom-right corner.
const MICROPHONE_ICON: u16 = 16;
const MICROPHONE_PADDING: u16 = 12;
const MICROPHONE_MARGIN: u16 = 16;
/// The live preview above the microphone while dictating.
const PREVIEW_PADDING: u16 = 8;
const PREVIEW_GAP: u16 = 8;
const PREVIEW_MAX_WIDTH: f32 = 320.0;
/// The close button in a dialog's top-right corner.
const DIALOG_CLOSE_ICON: u16 = 16;
const DIALOG_CLOSE_INSET: u16 = 8;
const MENU_ITEM_PADDING_Y: u16 = 8;
const MENU_ITEM_PADDING_X: u16 = 16;

/// Settings for every window opened with `window::open`.
///
/// `window::open` starts from iced's defaults, not from the settings libcosmic
/// gives its own main window (`iced_settings` in libcosmic's `app/mod.rs`).
/// The main window never flickered and note windows always did, so this
/// mirrors libcosmic's main-window setup: a transparent surface under the
/// opaque theme background, the application id, and no server-side title
/// bar - Fleck draws its own header bar, which holds the note menu.
pub(super) fn note_window_settings(size: Size) -> window::Settings {
    let mut settings = window::Settings {
        size,
        ..window::Settings::default()
    };
    settings.transparent = true;
    settings.decorations = false;
    settings.platform_specific.application_id = <Fleck as cosmic::Application>::APP_ID.to_string();
    settings
}

/// The minimum height to give the note body's editor+canvas stack, given the
/// finite viewport height `responsive` reports for it: the viewport height,
/// floored to a whole pixel and with the editor's own vertical padding
/// subtracted, so a short note's content is at most viewport-tall (filling
/// it with ruled lines) while a long note is free to grow past it and
/// scroll.
///
/// Subtracting the padding matters just as much as flooring: the editor's
/// `Shrink` layout takes `max(text height, min_height)` and then adds
/// `padding.y()` (top + bottom) *on top* of that (see `text_editor.rs`
/// ~686-697 in the pinned iced fork). Passing the bare viewport height
/// through as `min_height` therefore makes the editor's outer height
/// `viewport + 2 * BODY_PADDING` - always taller than the viewport, so the
/// `scrollable` around it always saw an overflow and always showed a
/// scrollbar, even for a note that fits. Subtracting `2 * BODY_PADDING`
/// first cancels that back out.
///
/// Flooring matters too: `responsive` can report a fractional viewport height
/// (e.g. 767.6). Passing that straight through as `min_height` makes the
/// editor's content exactly as tall as the viewport, and whether that
/// counts as "overflowing" the `scrollable` around it then comes down to
/// sub-pixel rounding in text layout - which flips from frame to frame,
/// toggling the scrollbar on and off and, with it, the note body's
/// scroll-offset clamping (see `Scrollable::layout` in the pinned iced
/// fork), producing a visible one-pixel jitter of the ruled canvas. Flooring
/// the minimum keeps it a whole pixel and strictly at or below the
/// (possibly fractional) viewport height, so the content can never be
/// measured as taller than the viewport and the scrollbar never appears for
/// a short note in the first place.
///
/// `responsive` sits outside the body's `scrollable` specifically so it only
/// ever sees a real, finite size (see `view_window`) - but a non-finite
/// input here is a caller bug away, and `min_height(f32::INFINITY)` would
/// mean "this editor is infinitely tall", i.e. the exact failure this
/// arrangement exists to avoid. So a non-finite `viewport_height` falls back
/// to no minimum (`0.0`) rather than passing infinity into layout.
pub(super) fn body_min_height(viewport_height: f32) -> f32 {
    if viewport_height.is_finite() {
        // The editor's `Shrink` layout adds `padding.y()` (top + bottom) on
        // top of whatever `min_height` it's given (see `text_editor.rs`
        // ~686-697 in the pinned iced fork), so passing the raw viewport
        // through here makes the editor's *outer* height
        // `viewport + 2 * BODY_PADDING` - always taller than the viewport,
        // which is exactly why the scrollable always saw an overflow and
        // showed a scrollbar even for a note that fits. Subtracting the
        // padding first cancels that back out, so `min_height + padding`
        // lands at (at most) the viewport height. Clamped at zero: a
        // viewport shorter than the padding must not go negative.
        (viewport_height.floor() - 2.0 * BODY_PADDING).max(0.0)
    } else {
        0.0
    }
}

/// Classifies a `text_editor::Action` for `WindowNote::history`: `None` for
/// actions that don't change the text (cursor moves, selections, clicks,
/// scrolling...), otherwise the `undo::EditKind` to record it under and
/// whether it must force a new undo group - see `undo`'s module docs for
/// the coalescing rule this implements. Paste, Enter, indent/unindent and a
/// whitespace character all force a boundary; an ordinary character or a
/// deletion may coalesce into the current group.
pub(super) fn edit_kind(action: &text_editor::Action) -> Option<(EditKind, bool)> {
    let text_editor::Action::Edit(edit) = action else {
        return None;
    };
    Some(match edit {
        text_editor::Edit::Insert(c) => (EditKind::Insert, c.is_whitespace()),
        text_editor::Edit::Enter
        | text_editor::Edit::Paste(_)
        | text_editor::Edit::Indent
        | text_editor::Edit::Unindent => (EditKind::Insert, true),
        text_editor::Edit::Backspace | text_editor::Edit::Delete => (EditKind::Delete, false),
    })
}

impl Fleck {
    /// A note window's content: see `view_window`, which routes every
    /// window that isn't the notes list here.
    pub(super) fn view_note(&self, id: window::Id) -> Element<'_, Message> {
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
        // is its first line of text until renamed, from the note's menu or
        // the notes list.

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
        let colour = self.note_colour(id);
        let body = self.note_body(id, colour);

        let header = self.note_header(id);

        let thumbnails = self.note_thumbnails(id);
        let microphone = self.note_microphone(id);
        let preview = self.dictation_preview(id).map(|text| {
            widget::container(widget::text::body(text.to_string()))
                .class(cosmic::theme::Container::Dialog(false))
                .padding(PREVIEW_PADDING)
                .max_width(PREVIEW_MAX_WIDTH)
        });

        // The paper starts where the title bar ends, like any other app's
        // content; the title bar keeps the theme's colours.
        // The microphone floats over the paper's bottom-right corner.
        let body = Stack::new()
            .push(
                widget::Column::with_capacity(2)
                    .push(body)
                    .push_maybe(thumbnails),
            )
            .push(
                widget::container(
                    widget::Column::with_capacity(2)
                        .spacing(PREVIEW_GAP)
                        .align_x(cosmic::iced::Alignment::End)
                        .push_maybe(preview)
                        .push(microphone),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(cosmic::iced::alignment::Horizontal::Right)
                .align_y(cosmic::iced::alignment::Vertical::Bottom)
                .padding(MICROPHONE_MARGIN),
            );
        let body = widget::container(body)
            .class(cosmic::theme::Container::custom(note_paper_style(colour)))
            // The strip lies over the paper, so the text starts below where
            // the resting bars sit rather than under them.
            .padding(cosmic::iced::Padding {
                top: f32::from(NOTE_PADDING_Y) + STUB_REST_HEIGHT,
                right: f32::from(NOTE_PADDING_X),
                bottom: f32::from(NOTE_PADDING_Y),
                left: f32::from(NOTE_PADDING_X),
            })
            .width(Length::Fill)
            .height(Length::Fill);
        // The strip goes over the paper *after* its padding, so the tabs sit
        // flush against the title bar rather than inset with the text.
        let body: Element<'_, Message> = match self.note_stubs(id) {
            Some(stubs) => Stack::new().push(body).push(stubs).into(),
            None => body.into(),
        };
        // Clicking in the note closes its menu. Presses on the open menu never
        // reach here: the menu is an overlay and takes them first.
        let body = widget::mouse_area(body).on_press(Message::NoteMenuClose(id));
        // Dropped files arrive this way on Wayland: the window toolkit's own
        // file-drop event is X11-only (see `Message::NoteFilesDropped`).
        let body = widget::dnd_destination(body, vec![images::URI_LIST.into()])
            .on_enter(move |_, _, mimes| Message::NoteDragEntered(id, mimes))
            .on_leave(move || Message::NoteDragLeft(id))
            .on_finish(move |_mime, data, _action, _, _| {
                Message::NoteUriListDropped(id, std::sync::Arc::new(data))
            });

        let note = widget::container(widget::Column::with_capacity(2).push(header).push(body))
            // No background of its own: the title bar and the paper each paint
            // one and together cover the whole window. A window-wide background
            // here doubled up behind the title bar, so COSMIC's translucent
            // frosted-glass background stacked twice and looked nearly solid.
            .width(Length::Fill)
            .height(Length::Fill);

        // A modal popover rather than `Application::dialog`, which libcosmic
        // only draws on the main window (the notes list). The note stays at
        // index 0 of the popover's children, so the editor keeps its state
        // whether or not a dialog is showing.
        let mut note = widget::popover(note).modal(true);
        if let Some(dialog) = self.note_dialog_view(id) {
            note = note.popup(dialog);
        }
        note.into()
    }

    /// Fleck's own header bar for a note window, like libcosmic's main
    /// window: the menu on the left, the note's name, then the window buttons.
    fn note_header(&self, id: window::Id) -> Element<'_, Message> {
        let menu_button = widget::button::text(crate::fl!("settings"))
            .height(Length::Fixed(f32::from(MENU_BUTTON_HEIGHT)))
            .padding([0, MENU_BUTTON_PADDING_X])
            .on_press(Message::NoteMenuToggle(id))
            .class(text_button_class());
        let mut menu = widget::popover(menu_button).position(widget::popover::Position::Point(
            cosmic::iced::Point::new(0.0, f32::from(MENU_BUTTON_HEIGHT)),
        ));
        // No `on_close`: the popover fires it on any press outside the
        // Settings button - including presses inside the menu - which closed the menu
        // before its items (which act on release) could respond. Clicking in
        // the note's text closes it instead (see `view_note`).
        if self.note_menu == Some(id) {
            menu = menu.popup(self.note_menu_popup(id));
        }

        let title = self
            .windows
            .get(&id)
            .and_then(|window| self.notes.get(&window.uuid))
            .map_or_else(
                || fleck_core::UNNAMED.to_string(),
                |note| display_name(note).to_string(),
            );
        let mut header = widget::header_bar()
            .title(title)
            .focused(self.core.focused_window() == Some(id))
            .start(menu)
            .on_drag(Message::NoteWindowDrag(id))
            .on_double_click(Message::NoteWindowMaximize(id))
            .on_close(Message::CloseRequested(id));
        if cosmic::config::show_maximize() {
            header = header.on_maximize(Message::NoteWindowMaximize(id));
        }
        if cosmic::config::show_minimize() {
            header = header.on_minimize(Message::NoteWindowMinimize(id));
        }
        // libcosmic's header bar is transparent. Painting the theme's window
        // background behind it keeps the title bar - title, Settings and
        // window buttons - in the COSMIC theme's own colours, with no rounding
        // at its bottom edge where the note's paper starts.
        widget::container(header)
            .class(cosmic::theme::Container::custom(note_title_bar_style))
            .width(Length::Fill)
            .into()
    }

    /// The colour of the note shown in window `id`; yellow if there isn't one.
    fn note_colour(&self, id: window::Id) -> Colour {
        self.windows
            .get(&id)
            .and_then(|window| self.notes.get(&window.uuid))
            .map_or(Colour::Default, |note| {
                Colour::from_name(&note.frontmatter.color)
            })
    }

    /// A note's body: the text editor over its ruled-paper canvas, inside a
    /// scrollable so the two move together.
    fn note_body(&self, id: window::Id, colour: Colour) -> Element<'_, Message> {
        let (input_id, content) = match self.windows.get(&id) {
            Some(window) => (window.input_id.clone(), &window.content),
            None => (self.fallback_input_id.clone(), &self.fallback_content),
        };
        widget::responsive(move |size| {
            let editor = text_editor::text_editor(content)
                .on_action(move |action| Message::BodyAction(id, action))
                .id(input_id.clone())
                // Intercepts Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y *before* the
                // editor's own default key handling
                // (`Binding::from_key_press`, called below as a fallback
                // for every other key) ever sees them - so Ctrl+Z can never
                // fall through to inserting a literal "z" or moving the
                // cursor. iced's `text_editor` has no undo/redo of its own
                // (see `undo`'s module docs); `Message::Undo`/`Redo` drive
                // the per-window `UndoHistory` built in `fleck/src/undo.rs`.
                .key_binding(move |press| {
                    let combo = press.key.to_latin(press.physical_key);
                    if press.modifiers.command() {
                        match combo {
                            Some('z') if press.modifiers.shift() => {
                                return Some(text_editor::Binding::Custom(Message::Redo(id)));
                            }
                            Some('z') => {
                                return Some(text_editor::Binding::Custom(Message::Undo(id)));
                            }
                            Some('y') => {
                                return Some(text_editor::Binding::Custom(Message::Redo(id)));
                            }
                            // An image on the clipboard becomes a picture in
                            // the note; anything else pastes as text (see
                            // `Message::NoteImagePasted`).
                            Some('v') => {
                                return Some(text_editor::Binding::Custom(
                                    Message::NotePasteImage(id),
                                ));
                            }
                            _ => {}
                        }
                    }
                    text_editor::Binding::from_key_press(press)
                })
                // No left padding: the text starts at the lined area's left
                // edge, in line with the Settings label above it. On the right
                // it stops short of the scrollbar, which only reserves
                // `SCROLLBAR_GAP` of its own - too little to keep a long line
                // from reading as though it runs into the bar. The ruled lines
                // behind the text still span the full width.
                .padding(cosmic::iced::Padding {
                    top: BODY_PADDING,
                    bottom: BODY_PADDING,
                    left: 0.0,
                    right: BODY_PADDING,
                })
                .size(BODY_TEXT_SIZE)
                .line_height(LineHeight::Absolute(Pixels(BODY_LINE_HEIGHT)))
                .min_height(body_min_height(size.height))
                .style(move |theme: &cosmic::Theme, _status| {
                    let value = colour
                        .text()
                        .map_or_else(|| Color::from(theme.current_container().on), rgb);
                    let mut placeholder = value;
                    placeholder.a *= 0.7;
                    text_editor::Style {
                        // Transparent: the canvas drawn behind it (pushed
                        // `push_under`, below) is what actually paints an
                        // opaque background for this area - see the warning
                        // in `ruled::RuledLines` about why that canvas fill
                        // has to exist at all.
                        background: Color::TRANSPARENT.into(),
                        border: Border {
                            width: 0.0,
                            ..Border::default()
                        },
                        placeholder,
                        value,
                        selection: Color::from(theme.cosmic().accent.base),
                    }
                });

            let lines = widget::canvas(RuledLines {
                line_height: BODY_LINE_HEIGHT,
                padding_top: BODY_PADDING,
                color: colour.line().map(rgb),
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
            let stack = Stack::new()
                .push(editor)
                .push_under(lines)
                .width(Length::Fill);

            // The `scrollable` lives *inside* `responsive`, wrapping the
            // editor+canvas stack: for a short note the stack is exactly
            // `size.height` tall (via `min_height` above) and doesn't
            // scroll, so the lines fill the window; for a long note the
            // stack grows past `size.height` and this `scrollable` is what
            // lets the editor and the ruled lines behind it scroll together
            // as a unit - if the editor scrolled *internally* instead, the
            // canvas would stay fixed while the text moved, breaking the
            // line alignment.
            let body = widget::scrollable(stack)
                .direction(cosmic::iced::widget::scrollable::Direction::Vertical(
                    super::list::slim_scrollbar(self.note_scroll_hovered == Some(id)),
                ))
                .width(Length::Fill)
                .height(Length::Fill)
                .class(super::list::SLIM_SCROLLBAR_CLASS);
            widget::mouse_area(body)
                .on_enter(Message::NoteScrollHover(id, true))
                .on_exit(Message::NoteScrollHover(id, false))
                .into()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// The dictation button in the note's bottom-right corner: a microphone
    /// that pulses while recording, and a spinner while Fleck works out what
    /// was said.
    fn note_microphone(&self, id: window::Id) -> Element<'_, Message> {
        let state = self.dictation_state(id);
        let icon = match state {
            DictationState::Transcribing => widget::icon::from_name("content-loading-symbolic")
                .size(MICROPHONE_ICON)
                .handle(),
            _ => crate::icons::microphone(),
        };
        widget::button::icon(icon)
            .icon_size(MICROPHONE_ICON)
            .padding(MICROPHONE_PADDING)
            .on_press(Message::VoiceToggle(id))
            .class(super::style::microphone_button_class(state))
            .into()
    }

    /// The row of thumbnails under a note's text, one per image it links to,
    /// in the order they appear. `None` when the note has no images.
    fn note_thumbnails(&self, id: window::Id) -> Option<Element<'_, Message>> {
        let uuid = self.windows.get(&id)?.uuid;
        let body = &self.notes.get(&uuid)?.body;
        let mut row = widget::Row::new().spacing(THUMBNAIL_GAP);
        let mut any = false;
        for (line, name) in fleck_core::image_links(body) {
            let Some(path) = self.store.image_path(uuid, &name) else {
                continue;
            };
            any = true;
            // A click puts the cursor on the image's line and opens the
            // picture full size (see `Message::NoteImageClicked`).
            row = row.push(
                widget::button::custom(
                    widget::image(widget::image::Handle::from_path(path))
                        .height(Length::Fixed(THUMBNAIL_HEIGHT)),
                )
                .class(menu_item_button_class())
                .padding(THUMBNAIL_PADDING)
                .on_press(Message::NoteImageClicked(id, line, name)),
            );
        }
        any.then(|| {
            // The thumbnail row's bar matches the note's, and widens with it:
            // the row sits directly under the body the mouse is already over.
            widget::scrollable(row)
                .direction(cosmic::iced::widget::scrollable::Direction::Horizontal(
                    super::list::slim_scrollbar(self.note_scroll_hovered == Some(id)),
                ))
                .width(Length::Fill)
                .class(super::list::SLIM_SCROLLBAR_CLASS)
                .into()
        })
    }

    /// One stub in a note's left strip: a bar of its note's colour, the note's
    /// name up the side of it once the strip is awake, and the panel it opens
    /// into when the mouse is on this one. `last` is the final tab in the
    /// strip, which draws no divider after itself.
    fn note_stub(&self, id: window::Id, note: &Note, last: bool) -> Element<'_, Message> {
        let uuid = note.frontmatter.uuid;
        let colour = Colour::from_name(&note.frontmatter.color);
        let name = display_name(note).to_string();
        let hovered = self.stub_hovered == Some((id, uuid));
        let awake = self.strip_hovered == Some(id);
        let label = |text: String, lines: usize| {
            widget::text::body(text)
                .class(colour.text().map_or(cosmic::theme::Text::Default, |text| {
                    cosmic::theme::Text::Color(rgb(text))
                }))
                .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(lines)))
        };
        // Asleep, a stub is a bar of colour with no name on it; awake, the name
        // is painted up it by a canvas, since iced can rotate a canvas's
        // contents but not a text widget's. Hovered, the stub becomes a panel
        // and the text reads the usual way round.
        let (face, padding): (Element<'_, Message>, u16) = if hovered {
            let preview = super::list::preview_lines(note, STUB_PANEL_LINES).join("\n");
            (
                widget::Column::with_capacity(2)
                    .spacing(STUB_PANEL_GAP)
                    .push(label(name, 1))
                    .push(label(preview, STUB_PANEL_LINES))
                    .into(),
                STUB_PADDING,
            )
        } else {
            let height = if awake {
                STUB_HEIGHT_AWAKE
            } else {
                STUB_REST_HEIGHT
            };
            (
                widget::canvas(crate::stub::Stub {
                    text: if awake { name } else { String::new() },
                    fill: colour.paper().map(rgb),
                    text_color: colour.text().map(rgb),
                    size: STUB_TEXT_SIZE,
                    last,
                    across: true,
                })
                .width(Length::Fixed(STUB_WIDTH))
                .height(Length::Fixed(height))
                .into(),
                0,
            )
        };
        let stub = widget::button::custom(face)
            .padding(padding)
            .width(Length::Fixed(if hovered {
                STUB_PANEL_WIDTH
            } else {
                STUB_WIDTH
            }))
            .height(Length::Fixed(if hovered {
                STUB_PANEL_HEIGHT
            } else if awake {
                STUB_HEIGHT_AWAKE
            } else {
                STUB_REST_HEIGHT
            }))
            .class(if hovered {
                card_button_class(colour)
            } else {
                // The canvas has already painted the stub, chamfers and all; a
                // button's own rectangle would square it off again.
                cosmic::theme::Button::Transparent
            })
            .on_press(Message::NoteRoute(id, uuid));
        widget::mouse_area(stub)
            .on_enter(Message::NoteStubHover(id, Some(uuid)))
            .on_exit(Message::NoteStubHover(id, None))
            .into()
    }

    /// The strip of stubs down the left edge of a note: one per other note,
    /// coloured like its card, carrying the start of its name.
    ///
    /// Hovering one slides it out into a panel with the whole name and the
    /// first lines of the note; clicking it shows that note in this window
    /// (`Message::NoteRoute`) rather than opening anything new. Notes that
    /// already have a window of their own are left out, so a click can never
    /// end with two windows editing the same note.
    fn note_stubs(&self, id: window::Id) -> Option<Element<'_, Message>> {
        // No strip for a window Fleck doesn't know about. The note this window
        // shows needs no excluding of its own: it is in `windows` too, so the
        // open set below covers it.
        self.windows.get(&id)?;
        let open: std::collections::HashSet<Uuid> =
            self.windows.values().map(|window| window.uuid).collect();
        let mut notes: Vec<&Note> = self
            .notes
            .values()
            .filter(|note| !open.contains(&note.frontmatter.uuid))
            .collect();
        if notes.is_empty() {
            return None;
        }
        notes.sort_by(|a, b| {
            self.note_mtime(b.frontmatter.uuid)
                .cmp(&self.note_mtime(a.frontmatter.uuid))
        });

        let final_tab = notes.len() - 1;
        let mut strip = widget::Row::with_capacity(notes.len());
        for (position, note) in notes.into_iter().enumerate() {
            strip = strip.push(self.note_stub(id, note, position == final_tab));
        }

        // The strip as a whole is what wakes the bars into named stubs; an
        // individual stub is what opens a panel.
        Some(
            widget::mouse_area(
                widget::container(
                    widget::scrollable(strip)
                        .id(strip_id(id))
                        .direction(cosmic::iced::widget::scrollable::Direction::Horizontal(
                            cosmic::iced::widget::scrollable::Scrollbar::hidden(),
                        ))
                        .on_scroll(move |viewport| {
                            Message::NoteStripScrolled(id, viewport.absolute_offset().x)
                        })
                        .width(Length::Fill),
                )
                .width(Length::Fill)
                // Only as tall as the strip needs: a container filling the
                // note would mean the mouse never left the strip, so it stayed
                // awake while you typed.
                .height(Length::Fixed(
                    if self.stub_hovered.is_some_and(|(w, _)| w == id) {
                        STUB_PANEL_HEIGHT
                    } else if self.strip_hovered == Some(id) {
                        STUB_HEIGHT_AWAKE
                    } else {
                        STUB_BAND
                    },
                ))
                .align_x(cosmic::iced::alignment::Horizontal::Left)
                .align_y(cosmic::iced::alignment::Vertical::Top),
            )
            .on_enter(Message::NoteStripHover(id, true))
            .on_exit(Message::NoteStripHover(id, false))
            .on_scroll(move |delta| {
                let movement = match delta {
                    cosmic::iced::mouse::ScrollDelta::Lines { x, y } => (x + y) * STUB_WHEEL_LINE,
                    cosmic::iced::mouse::ScrollDelta::Pixels { x, y } => x + y,
                };
                Message::NoteStripWheel(id, movement)
            })
            .into(),
        )
    }

    /// The menu under a note's settings button: rename, delete, back to the
    /// list, and the app-wide theme.
    fn note_menu_popup(&self, id: window::Id) -> Element<'_, Message> {
        let item = |label: String, message: Message| {
            widget::button::custom(widget::text::body(label))
                .class(menu_item_button_class())
                .padding([MENU_ITEM_PADDING_Y, MENU_ITEM_PADDING_X])
                .width(Length::Fill)
                .on_press(message)
        };
        let mut menu = widget::Column::with_capacity(8)
            .push(item(crate::fl!("edit-name"), Message::NoteRenameStart(id)))
            .push(item(
                crate::fl!("change-colour"),
                Message::NoteColourStart(id),
            ))
            .push(item(crate::fl!("remind-me"), Message::NoteRemindStart(id)))
            .push(item(
                crate::fl!("delete-note"),
                Message::NoteDeleteStart(id),
            ))
            .push(item(
                crate::fl!("back-to-list"),
                Message::NoteBackToList(id),
            ))
            .push(widget::divider::horizontal::default())
            .push(
                widget::container(widget::text::caption(crate::fl!("theme")))
                    .padding([MENU_ITEM_PADDING_Y, MENU_ITEM_PADDING_X]),
            );
        for theme in AppTheme::ALL {
            menu = menu.push(
                widget::container(widget::radio(
                    widget::text::body(theme.label()),
                    theme,
                    Some(self.theme),
                    Message::SetTheme,
                ))
                .padding([MENU_ITEM_PADDING_Y, MENU_ITEM_PADDING_X]),
            );
        }
        widget::container(menu)
            .class(cosmic::theme::Container::Dropdown)
            .padding(MENU_PADDING)
            .width(Length::Fixed(MENU_WIDTH))
            .into()
    }

    /// The colour dialog's swatch grid, and the dialog around it.
    fn colour_dialog<'a>(
        &'a self,
        id: window::Id,
        frame: widget::Dialog<'a, Message>,
    ) -> Element<'a, Message> {
        let current = self.note_colour(id);
        let mut grid = widget::Column::with_capacity(Colour::ALL.len().div_ceil(SWATCHES_PER_ROW))
            .spacing(SWATCH_GAP);
        for row in Colour::ALL.chunks(SWATCHES_PER_ROW) {
            let mut swatches = widget::Row::with_capacity(SWATCHES_PER_ROW).spacing(SWATCH_GAP);
            for &colour in row {
                // Full width, so the swatch and its label centre in the cell.
                let swatch = widget::Column::with_capacity(2)
                    .spacing(SWATCH_LABEL_GAP)
                    .width(Length::Fill)
                    .align_x(cosmic::iced::Alignment::Center)
                    .push(
                        widget::container(widget::text(""))
                            .width(Length::Fixed(SWATCH_SIZE))
                            .height(Length::Fixed(SWATCH_SIZE))
                            .class(cosmic::theme::Container::custom(swatch_style(
                                colour,
                                colour == current,
                            ))),
                    )
                    .push(widget::text::caption(colour.label()));
                swatches = swatches.push(
                    widget::button::custom(swatch)
                        .class(menu_item_button_class())
                        .padding(SWATCH_PADDING)
                        .width(Length::Fill)
                        .on_press(Message::NoteSetColour(id, colour)),
                );
            }
            // Pad a short last row with empty cells, so its swatches keep
            // the same width as the rows above.
            for _ in row.len()..SWATCHES_PER_ROW {
                swatches = swatches.push(widget::container(widget::text("")).width(Length::Fill));
            }
            grid = grid.push(swatches);
        }
        frame
            .title(crate::fl!("colour-title"))
            .control(grid)
            .apply(|dialog| Self::with_close_button(id, dialog.into()))
    }

    /// A dialog with a close button in its top-right corner, where every other
    /// window puts one. For the dialogs that only pick or dismiss; the rename
    /// and delete confirmations keep a Cancel button of their own.
    fn with_close_button(id: window::Id, dialog: Element<'_, Message>) -> Element<'_, Message> {
        let close = widget::button::icon(
            widget::icon::from_name("window-close-symbolic").size(DIALOG_CLOSE_ICON),
        )
        .extra_small()
        .on_press(Message::NoteDialogCancel(id))
        .class(icon_button_class(IconHoverRole::Accent));
        Stack::new()
            .push(dialog)
            .push(
                widget::container(close)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(cosmic::iced::alignment::Horizontal::Right)
                    .align_y(cosmic::iced::alignment::Vertical::Top)
                    .padding(DIALOG_CLOSE_INSET),
            )
            .into()
    }

    /// The rename or delete dialog open over note window `id`, if any.
    fn note_dialog_view(&self, id: window::Id) -> Option<Element<'_, Message>> {
        let (dialog_window, dialog) = self.note_dialog.as_ref()?;
        if *dialog_window != id {
            return None;
        }
        let uuid = self.windows.get(&id)?.uuid;
        let window_width = self
            .window_state
            .sizes
            .get(&uuid)
            .map_or(DEFAULT_WINDOW_SIZE.0, |&(width, _)| width);
        let frame = widget::dialog().width(Length::Fixed(dialog_width(window_width as f32)));
        Some(match dialog {
            NoteDialog::Rename(typed) => frame
                .title(crate::fl!("rename-title"))
                .control(
                    widget::text_input(crate::fl!("rename-placeholder"), typed.as_str())
                        .id(self.note_rename_input_id.clone())
                        .on_input(move |text| Message::NoteRenameInput(id, text))
                        .on_submit(move |_| Message::NoteRenameSave(id)),
                )
                .primary_action(
                    widget::button::suggested(crate::fl!("save"))
                        .on_press(Message::NoteRenameSave(id)),
                )
                .secondary_action(
                    widget::button::standard(crate::fl!("cancel"))
                        .on_press(Message::NoteDialogCancel(id)),
                )
                .into(),
            NoteDialog::Colour => self.colour_dialog(id, frame),
            NoteDialog::Image(name) => {
                // No title: the file name is a uuid, which says nothing and
                // runs under the close button. Bounded height so a big
                // picture can't push the dialog past the window.
                let image = self.store.image_path(uuid, name).map(|path| {
                    widget::image(widget::image::Handle::from_path(path))
                        .width(Length::Fill)
                        .height(Length::Fixed(IMAGE_DIALOG_MAX_HEIGHT))
                        .content_fit(cosmic::iced::ContentFit::Contain)
                });
                frame
                    .control(
                        widget::container(image.map_or_else(
                            || Element::from(widget::text::body(crate::fl!("image-missing"))),
                            Element::from,
                        ))
                        .center_x(Length::Fill),
                    )
                    .primary_action(
                        widget::button::suggested(crate::fl!("image-copy"))
                            .on_press(Message::NoteImageCopy(id, name.clone())),
                    )
                    .secondary_action(
                        widget::button::destructive(crate::fl!("image-delete"))
                            .on_press(Message::NoteImageRemove(id, name.clone())),
                    )
                    .apply(|dialog| Self::with_close_button(id, dialog.into()))
            }
            NoteDialog::Delete => {
                let name = self.notes.get(&uuid).map_or("", display_name);
                frame
                    .title(crate::fl!("delete-title"))
                    .body(crate::fl!("delete-body", name = name))
                    .primary_action(
                        widget::button::destructive(crate::fl!("delete-confirm"))
                            .on_press(Message::NoteDeleteConfirm(id)),
                    )
                    .secondary_action(
                        widget::button::standard(crate::fl!("cancel"))
                            .on_press(Message::NoteDialogCancel(id)),
                    )
                    .into()
            }
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact pixel values are the point of these tests
mod tests {
    use super::*;

    #[test]
    fn body_min_height_passes_through_a_finite_viewport_minus_padding() {
        assert_eq!(body_min_height(600.0), 600.0 - 2.0 * BODY_PADDING);
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

    #[test]
    fn body_min_height_floors_a_fractional_viewport_to_a_whole_pixel() {
        assert_eq!(body_min_height(767.6), 767.0 - 2.0 * BODY_PADDING);
    }

    /// The bug Fix 1 guards against: the editor's `Shrink` layout adds
    /// `padding.y()` on top of `min_height`, so the min height passed in
    /// must already have the padding subtracted, or the editor's outer
    /// height (min height + padding) always exceeds the viewport and the
    /// scrollable always shows a scrollbar - even for a note that fits.
    #[test]
    fn body_min_height_plus_padding_never_exceeds_a_whole_viewport() {
        let viewport = 768.0;
        let outer_height = body_min_height(viewport) + 2.0 * BODY_PADDING;
        assert!(
            outer_height <= viewport,
            "outer height {outer_height} exceeds viewport {viewport}"
        );
    }

    #[test]
    fn body_min_height_plus_padding_never_exceeds_a_fractional_viewport() {
        let viewport = 767.6;
        let outer_height = body_min_height(viewport) + 2.0 * BODY_PADDING;
        assert!(
            outer_height <= viewport,
            "outer height {outer_height} exceeds viewport {viewport}"
        );
    }

    #[test]
    fn body_min_height_clamps_to_zero_when_viewport_is_smaller_than_the_padding() {
        let viewport = BODY_PADDING; // smaller than 2 * BODY_PADDING
        assert_eq!(body_min_height(viewport), 0.0);
    }

    #[test]
    fn body_min_height_never_exceeds_a_fractional_viewport_height() {
        let viewport = 767.6;
        assert!(
            body_min_height(viewport) <= viewport,
            "a short note's content must never be measured as taller than the viewport, \
             or the scrollable flips between overflowing and not from frame to frame"
        );
    }

    #[test]
    fn body_min_height_is_always_a_whole_pixel_for_a_finite_input() {
        assert_eq!(body_min_height(600.3).fract(), 0.0);
        assert_eq!(body_min_height(600.0).fract(), 0.0);
    }
}
