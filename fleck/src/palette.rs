//! Note colours: Default (the COSMIC theme's own look), classic sticky-note
//! yellow, and colours people know from Linux distributions. A coloured note's
//! paper and card use the colour exactly as it is, in light and dark mode
//! alike; its dotted lines use a darker shade, and its text is dark or white,
//! whichever reads better on that colour.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colour {
    /// The theme's own background, text and line colours.
    Default,
    Yellow,
    Pop,
    Ubuntu,
    Debian,
    Fedora,
    OpenSuse,
    Arch,
    Manjaro,
    Mint,
}

/// The dotted lines' brightness relative to the paper: each channel is
/// scaled by this factor.
const LINE_SHADE: f32 = 0.7;
const DARK_TEXT: (u8, u8, u8) = (0x1A, 0x1A, 0x1A);
const LIGHT_TEXT: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);

impl Colour {
    /// In the order the colour dialog shows them. Default first.
    pub const ALL: [Colour; 10] = [
        Colour::Default,
        Colour::Yellow,
        Colour::Pop,
        Colour::Ubuntu,
        Colour::Debian,
        Colour::Fedora,
        Colour::OpenSuse,
        Colour::Arch,
        Colour::Manjaro,
        Colour::Mint,
    ];

    /// The name stored in a note's frontmatter.
    pub fn name(self) -> &'static str {
        match self {
            Colour::Default => "default",
            Colour::Yellow => "yellow",
            Colour::Pop => "pop",
            Colour::Ubuntu => "ubuntu",
            Colour::Debian => "debian",
            Colour::Fedora => "fedora",
            Colour::OpenSuse => "opensuse",
            Colour::Arch => "arch",
            Colour::Manjaro => "manjaro",
            Colour::Mint => "mint",
        }
    }

    /// The colour for a stored name. Anything unknown, including names from
    /// Fleck's earlier palette, is Default.
    pub fn from_name(name: &str) -> Colour {
        Colour::ALL
            .into_iter()
            .find(|c| c.name() == name)
            .unwrap_or(Colour::Default)
    }

    /// The label shown in the colour dialog.
    pub fn label(self) -> String {
        match self {
            Colour::Default => crate::fl!("colour-default"),
            Colour::Yellow => crate::fl!("colour-yellow"),
            Colour::Pop => crate::fl!("colour-pop"),
            Colour::Ubuntu => crate::fl!("colour-ubuntu"),
            Colour::Debian => crate::fl!("colour-debian"),
            Colour::Fedora => crate::fl!("colour-fedora"),
            Colour::OpenSuse => crate::fl!("colour-opensuse"),
            Colour::Arch => crate::fl!("colour-arch"),
            Colour::Manjaro => crate::fl!("colour-manjaro"),
            Colour::Mint => crate::fl!("colour-mint"),
        }
    }

    /// The note's paper and card colour: classic sticky-note yellow, or the
    /// distribution's own colour, the same in light and dark mode. `None` for
    /// Default, which uses the theme's.
    pub fn paper(self) -> Option<(u8, u8, u8)> {
        Some(match self {
            Colour::Default => return None,
            Colour::Yellow => (0xF8, 0xE4, 0x8C),
            Colour::Pop => (0x48, 0xB9, 0xC7),
            Colour::Ubuntu => (0xE9, 0x54, 0x20),
            Colour::Debian => (0xD7, 0x0A, 0x53),
            Colour::Fedora => (0x51, 0xA2, 0xDA),
            Colour::OpenSuse => (0x73, 0xBA, 0x25),
            Colour::Arch => (0x17, 0x93, 0xD1),
            Colour::Manjaro => (0x35, 0xBF, 0xA4),
            Colour::Mint => (0x87, 0xCF, 0x3E),
        })
    }

    /// The dotted lines: a darker shade of the paper. `None` for Default.
    pub fn line(self) -> Option<(u8, u8, u8)> {
        let (r, g, b) = self.paper()?;
        let shade = |c: u8| (f32::from(c) * LINE_SHADE).round() as u8;
        Some((shade(r), shade(g), shade(b)))
    }

    /// The note's text: dark or white, whichever contrasts more with the
    /// paper. `None` for Default.
    pub fn text(self) -> Option<(u8, u8, u8)> {
        self.paper().map(readable_on)
    }

    /// Text on a card's darker heading strip (the same shade as the lines):
    /// dark or white, whichever contrasts more with that shade. `None` for
    /// Default.
    pub fn heading_text(self) -> Option<(u8, u8, u8)> {
        self.line().map(readable_on)
    }
}

/// Dark or white, whichever contrasts more with `background`.
fn readable_on(background: (u8, u8, u8)) -> (u8, u8, u8) {
    if contrast_ratio(DARK_TEXT, background) >= contrast_ratio(LIGHT_TEXT, background) {
        DARK_TEXT
    } else {
        LIGHT_TEXT
    }
}

impl Colour {}

/// WCAG 2.1 relative luminance.
fn relative_luminance((r, g, b): (u8, u8, u8)) -> f32 {
    fn channel(v: u8) -> f32 {
        let v = f32::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// WCAG 2.1 contrast ratio between two colours.
fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let (a, b) = (relative_luminance(a), relative_luminance(b));
    let (lighter, darker) = if a > b { (a, b) } else { (b, a) };
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colours_text_meets_wcag_aa() {
        for colour in Colour::ALL.into_iter().filter(|c| *c != Colour::Default) {
            let ratio = contrast_ratio(colour.text().unwrap(), colour.paper().unwrap());
            assert!(
                ratio >= 4.5,
                "{colour:?} text has contrast {ratio:.2}, need 4.5"
            );
        }
    }

    #[test]
    fn every_colours_heading_text_meets_wcag_aa_for_large_text() {
        // The heading strip's title is heading-sized, so AA's large-text 3:1.
        for colour in Colour::ALL.into_iter().filter(|c| *c != Colour::Default) {
            let ratio = contrast_ratio(colour.heading_text().unwrap(), colour.line().unwrap());
            assert!(
                ratio >= 3.0,
                "{colour:?} heading has contrast {ratio:.2}, need 3"
            );
        }
    }

    #[test]
    fn debian_red_gets_white_text_and_yellow_gets_dark_text() {
        assert_eq!(Colour::Debian.text(), Some(LIGHT_TEXT));
        assert_eq!(Colour::Yellow.text(), Some(DARK_TEXT));
    }

    #[test]
    fn lines_are_a_darker_shade_of_the_paper() {
        for colour in Colour::ALL.into_iter().filter(|c| *c != Colour::Default) {
            assert!(
                relative_luminance(colour.line().unwrap())
                    < relative_luminance(colour.paper().unwrap())
            );
        }
    }

    #[test]
    fn contrast_ratio_matches_known_values() {
        assert!((contrast_ratio((0, 0, 0), (255, 255, 255)) - 21.0).abs() < 0.01);
        assert!((contrast_ratio((120, 90, 200), (120, 90, 200)) - 1.0).abs() < 0.01);
    }

    #[test]
    fn every_colour_has_a_distinct_name_that_round_trips() {
        for colour in Colour::ALL {
            assert_eq!(Colour::from_name(colour.name()), colour);
        }
        let mut names: Vec<&str> = Colour::ALL.iter().map(|c| c.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Colour::ALL.len());
    }

    #[test]
    fn unknown_and_old_palette_names_are_default() {
        for name in ["", "green", "pink", "purple", "grey", "nonsense"] {
            assert_eq!(Colour::from_name(name), Colour::Default);
        }
    }

    #[test]
    fn default_takes_every_colour_from_the_theme() {
        assert_eq!(Colour::Default.paper(), None);
        assert_eq!(Colour::Default.line(), None);
        assert_eq!(Colour::Default.text(), None);
    }
}
