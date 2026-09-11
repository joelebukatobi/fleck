//! The ruled-paper look of a note's body: ruled.rs owns the pure arithmetic
//! that decides where each dotted line goes, plus the `canvas::Program` that
//! paints them. The canvas paints only the dotted lines - the opaque
//! background the transparent `text_editor` layered on top of it needs
//! comes from the window content's own `Container::WindowBackground` in
//! `app.rs`'s `view_window`, not from this canvas.
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

/// Draws the lined-paper background behind a note's body: one dotted
/// horizontal rule per line, positioned by [`line_offsets`] and coloured
/// from the current theme. Paints no fill of its own - the window content
/// it sits on is already opaque (see `app.rs`'s `view_window`), so this
/// canvas only ever needs to add the lines.
pub struct RuledLines {
    /// Must equal the `text_editor`'s own `line_height`, in pixels - see
    /// `app::BODY_LINE_HEIGHT`.
    pub line_height: f32,
    /// Must equal the `text_editor`'s own top padding, in pixels - see
    /// `app::BODY_PADDING`.
    pub padding_top: f32,
}

impl<Message> canvas::Program<Message, Theme> for RuledLines {
    // A `canvas::Cache`, not `()`: `draw` below is called on every cursor
    // blink and every keystroke (the note body redraws on each), and
    // without a cache each of those re-tessellates the whole dashed-line
    // background from scratch. `Cache::draw_with_bounds` already skips
    // redrawing when `bounds` is unchanged from the previous call, so a
    // resize (bounds change) still redraws - no separate invalidation
    // needed here. This has to live in `State`, not on the `RuledLines`
    // struct itself, because `view_window` rebuilds a fresh `RuledLines`
    // every `view()` call; `State` is what libcosmic keeps alive across
    // views.
    type State = canvas::Cache;

    fn draw(
        &self,
        state: &canvas::Cache,
        renderer: &cosmic::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<cosmic::Renderer>> {
        // Local bounds (size only, no position): the geometry itself never
        // depends on where the canvas sits in the window, only on how big
        // it is, so keying the cache on size alone (rather than the
        // `bounds` the canvas widget reports, whose x/y shifts whenever an
        // ancestor's layout does) avoids spurious cache misses.
        let local_bounds = Rectangle::with_size(bounds.size());
        let geometry = state.draw_with_bounds(renderer, local_bounds, |frame| {
            let container = theme.current_container();

            let dash = [1.0_f32, 3.0];
            let stroke = canvas::Stroke {
                style: canvas::Style::Solid(Color::from(container.divider)),
                width: 1.0,
                line_dash: canvas::LineDash {
                    segments: &dash,
                    offset: 0,
                },
                ..canvas::Stroke::default()
            };

            for y in line_offsets(bounds.height, self.line_height, self.padding_top) {
                let line = canvas::Path::line(Point::new(0.0, y), Point::new(bounds.width, y));
                frame.stroke(&line, stroke);
            }
        });

        vec![geometry]
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact pixel values are the point of these tests
mod tests {
    use super::*;

    #[test]
    fn first_line_accounts_for_padding() {
        let offsets = line_offsets(200.0, 24.0, 8.0);
        assert_eq!(
            offsets[0],
            8.0 + 24.0,
            "first rule sits one line below the padded top"
        );
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
        assert!(
            offsets.len() > 2,
            "need at least a few lines to check spacing between them"
        );
        for pair in offsets.windows(2) {
            assert_eq!(pair[1] - pair[0], 24.0);
        }
    }

    #[test]
    fn enough_lines_are_produced_to_fill_the_height() {
        let height = 100.0;
        let offsets = line_offsets(height, 24.0, 8.0);
        let last = *offsets
            .last()
            .expect("a positive height produces at least one line");
        assert!(
            last >= height,
            "last line {last} must reach all the way to {height}"
        );
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
