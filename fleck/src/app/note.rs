//! A note window: the body editor over its ruled-paper canvas.

use cosmic::iced::core::text::LineHeight;
use cosmic::iced::widget::Stack;
use cosmic::iced::{window, Border, Color, Length, Pixels, Size};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::text_editor;

use super::{Fleck, Message};
use crate::ruled::RuledLines;
use crate::undo::EditKind;

/// Size of the note body's text, in logical pixels.
pub(super) const BODY_TEXT_SIZE: f32 = 14.0;

/// Height of one line of the note body, in logical pixels. Set explicitly
/// (rather than left to font metrics) and shared, unchanged, with
/// `ruled::RuledLines::line_height` so the dotted rules drawn behind the
/// body can never drift out of alignment with the text sitting on them.
pub(super) const BODY_LINE_HEIGHT: f32 = 22.0;

/// The body `text_editor`'s padding, in logical pixels, on every side.
/// Shared with `ruled::RuledLines::padding_top` for the same reason as
/// `BODY_LINE_HEIGHT` - the first rule's offset has to account for exactly
/// this much space above the first line of text.
pub(super) const BODY_PADDING: f32 = 8.0;

/// Settings for every window opened with `window::open`.
///
/// `window::open` starts from iced's defaults, not from the settings libcosmic
/// gives its own main window (`iced_settings` in libcosmic's `app/mod.rs`).
/// The main window never flickered and note windows always did, so this
/// mirrors libcosmic's main-window setup: a transparent surface under the
/// opaque theme background, plus the application id. Decorations stay
/// server-side - a client-side header made the flicker far worse.
pub(super) fn note_window_settings(size: Size) -> window::Settings {
    let mut settings = window::Settings { size, ..window::Settings::default() };
    settings.transparent = true;
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
        text_editor::Edit::Enter | text_editor::Edit::Paste(_) => (EditKind::Insert, true),
        text_editor::Edit::Indent | text_editor::Edit::Unindent => (EditKind::Insert, true),
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
                            _ => {}
                        }
                    }
                    text_editor::Binding::from_key_press(press)
                })
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

        widget::container(body)
        // An explicit opaque background is a rendering requirement, not
        // decoration: `view_window` is used directly for every secondary
        // note window with nothing else wrapping it (see `Cosmic::view` in
        // libcosmic), so if this container's background were left at its
        // default (`Container::Transparent`), the whole window would render
        // transparent - the desktop showing through, stale frames smearing,
        // exactly the failure mode this task's brief warns about.
        .class(cosmic::theme::Container::WindowBackground)
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

#[cfg(test)]
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
        assert!(outer_height <= viewport, "outer height {outer_height} exceeds viewport {viewport}");
    }

    #[test]
    fn body_min_height_plus_padding_never_exceeds_a_fractional_viewport() {
        let viewport = 767.6;
        let outer_height = body_min_height(viewport) + 2.0 * BODY_PADDING;
        assert!(outer_height <= viewport, "outer height {outer_height} exceeds viewport {viewport}");
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
