//! A tab in a note's strip: a bar of its note's colour, with the note's name
//! on it once the strip is awake.
//!
//! The tabs paint themselves rather than sitting in buttons, because a strip
//! down the side of a note needs its names turned a quarter turn, and iced can
//! rotate a canvas's contents but not a text widget's. Kept beside `ruled` for
//! the same reason: the arithmetic is testable with no widget tree involved.

use cosmic::iced::widget::canvas;
use cosmic::iced::{mouse, Color, Point, Rectangle};
use cosmic::Theme;

/// The line between one tab and the next.
const SEPARATOR_ALPHA: f32 = 0.25;

/// How much of a name fits along a tab `along` pixels long, at `text_size`.
///
/// A rough measure on purpose: the glyphs are proportional, so this is the
/// average-width estimate every tab strip makes, and the ellipsis is there
/// for when it is wrong. A name that fits comes back whole.
#[must_use]
pub fn fit(name: &str, along: f32, text_size: f32) -> String {
    // Lowercase Latin text averages near 0.55 em; the ellipsis then covers the
    // rest. Below one character there is nothing useful to show.
    let per_glyph = text_size * 0.55;
    let fits = ((along / per_glyph).floor() as usize).max(1);
    let count = name.chars().count();
    if count <= fits {
        return name.to_string();
    }
    let kept: String = name.chars().take(fits.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// One tab: its colour, and - once the strip is awake - the note's name on it.
pub struct Stub {
    /// The note's name. Empty while the strip is at rest and the tabs are
    /// nothing but bars of colour.
    pub text: String,
    /// The note's own colour, or `None` for Default - which takes the theme's
    /// card background, the same as that note's card in the list.
    pub fill: Option<Color>,
    pub text_color: Option<Color>,
    pub size: f32,
    /// The last tab in the strip draws no divider: nothing follows it.
    pub last: bool,
    /// Whether the strip runs across the top of the note rather than down its
    /// side. Across, the name reads the usual way round and the divider is a
    /// vertical line; down the side, the name reads upwards and the divider
    /// lies along the bottom.
    pub across: bool,
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

        frame.fill_rectangle(Point::ORIGIN, bounds.size(), fill);

        // Tabs sit flush against each other, so the only thing dividing one
        // from the next is this line along the edge they share.
        if !self.last {
            let line = if self.across {
                canvas::Path::line(
                    Point::new(bounds.width, 0.0),
                    Point::new(bounds.width, bounds.height),
                )
            } else {
                canvas::Path::line(
                    Point::new(0.0, bounds.height),
                    Point::new(bounds.width, bounds.height),
                )
            };
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

        if self.text.is_empty() {
            return vec![frame.into_geometry()];
        }

        frame.translate(cosmic::iced::Vector::new(
            bounds.width / 2.0,
            bounds.height / 2.0,
        ));
        // Down the side, the name is turned a quarter turn anticlockwise so it
        // reads upwards; across the top it reads as text normally does.
        let along = if self.across {
            bounds.width
        } else {
            frame.rotate(cosmic::iced::Radians(-std::f32::consts::FRAC_PI_2));
            bounds.height
        };
        frame.fill_text(canvas::Text {
            content: fit(&self.text, along, self.size),
            position: Point::ORIGIN,
            color: text_color,
            size: self.size.into(),
            // The frame is centred on the tab (and rotated, down the side), so
            // centring the text's own box puts it down the middle of the tab.
            align_x: cosmic::iced::alignment::Horizontal::Center.into(),
            align_y: cosmic::iced::alignment::Vertical::Center,
            max_width: along,
            ..canvas::Text::default()
        });

        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
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
    fn a_tab_too_short_for_anything_still_shows_something() {
        assert!(!fit("Workada", 1.0, 14.0).is_empty());
    }
}
