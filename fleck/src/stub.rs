//! A stub in a note's left strip: its shape, and the note's name written up
//! the side of it.
//!
//! The stub paints itself rather than sitting in a button's rectangle: the
//! strip's outer corners are cut on a diagonal, which a rectangle cannot do,
//! and iced can rotate a canvas's contents but not a `text` widget's. Kept
//! beside `ruled` for the same reason: the arithmetic is testable with no
//! widget tree involved at all.

use cosmic::iced::widget::canvas;
use cosmic::iced::{mouse, Color, Point, Rectangle};
use cosmic::Theme;

/// How far the diagonal cuts into the strip's first and last stub.
pub const CHAMFER: f32 = 10.0;
/// The line between one stub and the next.
const SEPARATOR_ALPHA: f32 = 0.25;

/// How much of a name fits up a stub `height` pixels tall, at `text_size`.
///
/// A rough measure on purpose: the glyphs are proportional, so this is the
/// average-width estimate every tab strip makes, and the ellipsis is there
/// for when it is wrong. A name that fits comes back whole.
#[must_use]
pub fn fit(name: &str, height: f32, text_size: f32) -> String {
    // Lowercase Latin text averages near 0.55 em; the ellipsis then covers the
    // rest. Below one character there is nothing useful to show.
    let per_glyph = text_size * 0.55;
    let fits = ((height / per_glyph).floor() as usize).max(1);
    let count = name.chars().count();
    if count <= fits {
        return name.to_string();
    }
    let kept: String = name.chars().take(fits.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// The outline of a stub: a rectangle whose top edge (on the first stub) and
/// bottom edge (on the last) is a single diagonal running the whole width,
/// sloping in towards the note rather than out towards the window edge. The
/// strip reads as something tucked under the paper, and there is no straight
/// run of edge leading into the diagonal.
#[must_use]
pub fn outline(width: f32, height: f32, first: bool, last: bool) -> Vec<Point> {
    let cut = CHAMFER.min(height / 2.0);
    vec![
        Point::new(0.0, 0.0),
        Point::new(width, if first { cut } else { 0.0 }),
        Point::new(width, if last { height - cut } else { height }),
        Point::new(0.0, height),
    ]
}

/// One stub: its shape, its colours, and - once the strip is awake - the name
/// painted up it.
pub struct Stub {
    /// The note's name, painted up the stub. Empty while the strip is at rest
    /// and the stubs are nothing but bars of colour.
    pub text: String,
    /// The note's own colour, or `None` for Default - which takes the theme's
    /// card background, the same as that note's card in the list.
    pub fill: Option<Color>,
    pub text_color: Option<Color>,
    pub size: f32,
    pub first: bool,
    pub last: bool,
}

impl<Message> canvas::Program<Message, Theme> for Stub {
    type State = ();

    fn draw(
        &self,
        (): &(),
        renderer: &cosmic::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<cosmic::Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let text_color = self
            .text_color
            .unwrap_or_else(|| Color::from(theme.current_container().on));
        let fill = self
            .fill
            .unwrap_or_else(|| Color::from(theme.current_container().component.base));

        let mut path = canvas::path::Builder::new();
        let points = outline(bounds.width, bounds.height, self.first, self.last);
        path.move_to(points[0]);
        for point in &points[1..] {
            path.line_to(*point);
        }
        path.close();
        frame.fill(&path.build(), fill);

        // Stubs sit flush against each other, so the only thing dividing one
        // from the next is this line along the bottom.
        if !self.last {
            let line = canvas::Path::line(
                Point::new(0.0, bounds.height),
                Point::new(bounds.width, bounds.height),
            );
            frame.stroke(
                &line,
                canvas::Stroke {
                    style: canvas::Style::Solid(Color {
                        a: SEPARATOR_ALPHA,
                        ..text_color
                    }),
                    width: 1.0,
                    ..canvas::Stroke::default()
                },
            );
        }

        // A quarter turn anticlockwise about the stub's centre, so the name
        // runs from the bottom of the stub to the top - the way a vertical tab
        // reads.
        frame.translate(cosmic::iced::Vector::new(
            bounds.width / 2.0,
            bounds.height / 2.0,
        ));
        if self.text.is_empty() {
            return vec![frame.into_geometry()];
        }
        frame.rotate(cosmic::iced::Radians(-std::f32::consts::FRAC_PI_2));
        frame.fill_text(canvas::Text {
            content: fit(&self.text, bounds.height, self.size),
            position: Point::ORIGIN,
            color: text_color,
            size: self.size.into(),
            // The frame is rotated, so the text's own box is too: centring it
            // on the origin puts it down the middle of the stub.
            align_x: cosmic::iced::alignment::Horizontal::Center.into(),
            align_y: cosmic::iced::alignment::Vertical::Center,
            max_width: bounds.height,
            ..canvas::Text::default()
        });

        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact pixel values are the point of these tests
mod tests {
    use super::*;

    #[test]
    fn a_short_name_is_left_alone() {
        assert_eq!(fit("Songs", 200.0, 14.0), "Songs");
    }

    #[test]
    fn a_long_name_ends_in_an_ellipsis() {
        let name = "Meta questions for the interview";

        let fitted = fit(name, 60.0, 14.0);

        assert!(fitted.ends_with('…'), "{fitted}");
        assert!(fitted.chars().count() < name.chars().count());
    }

    #[test]
    fn a_stub_too_short_for_anything_still_shows_something() {
        assert!(!fit("Workada", 1.0, 14.0).is_empty());
    }

    #[test]
    fn a_stub_in_the_middle_of_the_strip_is_square() {
        let points = outline(24.0, 120.0, false, false);

        assert_eq!(
            points,
            vec![
                Point::new(0.0, 0.0),
                Point::new(24.0, 0.0),
                Point::new(24.0, 120.0),
                Point::new(0.0, 120.0),
            ]
        );
    }

    #[test]
    fn the_first_stub_slopes_down_towards_the_note() {
        let points = outline(24.0, 120.0, true, false);

        // The window edge keeps its square corner; the inner edge drops away.
        assert_eq!(points[0], Point::new(0.0, 0.0));
        assert_eq!(points[1], Point::new(24.0, CHAMFER));
    }

    #[test]
    fn the_last_stub_slopes_up_towards_the_note() {
        let points = outline(24.0, 120.0, false, true);

        assert_eq!(points[2], Point::new(24.0, 120.0 - CHAMFER));
        assert_eq!(points[3], Point::new(0.0, 120.0));
    }

    #[test]
    fn a_short_stub_cannot_cut_past_its_own_middle() {
        let points = outline(24.0, 10.0, true, true);

        assert!(
            points.iter().all(|p| p.y >= 0.0 && p.y <= 10.0),
            "{points:?}"
        );
    }
}
