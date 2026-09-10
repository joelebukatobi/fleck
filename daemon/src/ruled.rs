//! The ruled-paper look of a note's body: ruled.rs owns the pure arithmetic
//! that decides where each dotted line goes, plus the `canvas::Program` that
//! paints them (and the opaque background the transparent `text_editor`
//! layered on top of it depends on).
//!
//! Kept separate from `app.rs` so the position arithmetic can be unit
//! tested with no widget tree involved at all.

use cosmic::iced::widget::canvas;
use cosmic::iced::{mouse, Color, Point, Rectangle};
use cosmic::Theme;

/// The y-coordinate (in the canvas's own local space, same origin as the
/// `text_editor` it sits behind) of every ruled line needed to cover
/// `height` pixels, given the editor's `line_height` and its `padding_top`
/// (the gap between the editor's top edge and the top of its first line of
/// text).
///
/// Line `i` (0-indexed) underlines the text row occupying
/// `[padding_top + i*line_height, padding_top + (i+1)*line_height)`, so its
/// rule is drawn at `padding_top + (i+1)*line_height` - the bottom of that
/// row, which is where the text painted by the editor actually sits.
///
/// Enough lines are produced to cover `height`, including one final partial
/// line if `height` doesn't land exactly on a line boundary: dropping it
/// would leave a gap of bare, unruled space at the bottom of a resized
/// window.
///
/// A non-positive `height` or `line_height` produces no lines, rather than
/// an empty-looking page or an infinite loop.
pub fn line_offsets(height: f32, line_height: f32, padding_top: f32) -> Vec<f32> {
    if height <= 0.0 || line_height <= 0.0 {
        return Vec::new();
    }
    let mut offsets = Vec::new();
    let mut y = padding_top + line_height;
    while y - line_height < height {
        offsets.push(y);
        y += line_height;
    }
    offsets
}

/// Draws the lined-paper background behind a note's body: an opaque fill
/// (the transparent `text_editor` stacked on top of this canvas has nothing
/// else to paint the note's background - see the warning in `app.rs`'s
/// `view_window`) plus one dotted horizontal rule per line, positioned by
/// [`line_offsets`] and coloured from the current theme.
pub struct RuledLines {
    /// Must equal the `text_editor`'s own `line_height`, in pixels - see
    /// `app::BODY_LINE_HEIGHT`.
    pub line_height: f32,
    /// Must equal the `text_editor`'s own top padding, in pixels - see
    /// `app::BODY_PADDING`.
    pub padding_top: f32,
}

impl<Message> canvas::Program<Message, Theme> for RuledLines {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &cosmic::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<cosmic::Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let container = theme.current_container();

        // The opaque backing the module doc above promises: without this,
        // the transparent editor above would let whatever the compositor
        // clears the surface to (often nothing at all) show through.
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), Color::from(container.base));

        let dash = [1.0_f32, 3.0];
        let stroke = canvas::Stroke {
            style: canvas::Style::Solid(Color::from(container.divider)),
            width: 1.0,
            line_dash: canvas::LineDash { segments: &dash, offset: 0 },
            ..canvas::Stroke::default()
        };

        for y in line_offsets(bounds.height, self.line_height, self.padding_top) {
            let line = canvas::Path::line(Point::new(0.0, y), Point::new(bounds.width, y));
            frame.stroke(&line, stroke);
        }

        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_accounts_for_padding() {
        let offsets = line_offsets(200.0, 24.0, 8.0);
        assert_eq!(offsets[0], 8.0 + 24.0, "first rule sits one line below the padded top");
    }

    #[test]
    fn a_different_padding_shifts_every_line_by_the_same_amount() {
        let a = line_offsets(200.0, 24.0, 8.0);
        let b = line_offsets(200.0, 24.0, 20.0);
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(&b) {
            assert_eq!(b - a, 12.0);
        }
    }

    #[test]
    fn spacing_is_exactly_the_line_height() {
        let offsets = line_offsets(200.0, 24.0, 8.0);
        assert!(offsets.len() > 2, "need at least a few lines to check spacing between them");
        for pair in offsets.windows(2) {
            assert_eq!(pair[1] - pair[0], 24.0);
        }
    }

    #[test]
    fn enough_lines_are_produced_to_fill_the_height() {
        let height = 100.0;
        let offsets = line_offsets(height, 24.0, 8.0);
        let last = *offsets.last().expect("a positive height produces at least one line");
        assert!(last >= height, "last line {last} must reach all the way to {height}");
        // Not merely "enough", but no more than necessary: the line before
        // it must not already have reached the bottom, or this would also
        // pass for an implementation that pads with extra unneeded lines.
        if let Some(&second_last) = offsets.get(offsets.len().wrapping_sub(2)) {
            if offsets.len() > 1 {
                assert!(second_last < height);
            }
        }
    }

    #[test]
    fn zero_height_produces_no_lines() {
        assert!(line_offsets(0.0, 24.0, 8.0).is_empty());
    }

    #[test]
    fn negative_height_produces_no_lines_not_a_panic() {
        assert!(line_offsets(-50.0, 24.0, 8.0).is_empty());
    }

    #[test]
    fn zero_line_height_produces_no_lines_not_an_infinite_loop() {
        assert!(line_offsets(200.0, 0.0, 8.0).is_empty());
    }

    #[test]
    fn negative_line_height_produces_no_lines_not_an_infinite_loop() {
        assert!(line_offsets(200.0, -5.0, 8.0).is_empty());
    }
}
