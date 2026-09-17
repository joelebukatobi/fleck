//! Note colours: classic sticky-note yellow plus colours people know from
//! Linux distributions. A note's paper and its card use the colour exactly as
//! it is, in light and dark mode alike; its dotted lines use a darker shade,
//! and its text is dark or white, whichever reads better on that colour.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colour {
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
    /// In the order the colour dialog shows them. Yellow first: the default.
    pub const ALL: [Colour; 9] = [
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
    /// Fleck's earlier palette, is yellow.
    pub fn from_name(name: &str) -> Colour {
        Colour::ALL
            .into_iter()
            .find(|c| c.name() == name)
            .unwrap_or(Colour::Yellow)
    }

    /// The label shown in the colour dialog.
    pub fn label(self) -> String {
        match self {
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
    /// distribution's own colour. The same in light and dark mode.
    pub fn paper(self) -> (u8, u8, u8) {
        match self {
            Colour::Yellow => (0xF8, 0xE4, 0x8C),
            Colour::Pop => (0x48, 0xB9, 0xC7),
            Colour::Ubuntu => (0xE9, 0x54, 0x20),
            Colour::Debian => (0xD7, 0x0A, 0x53),
            Colour::Fedora => (0x51, 0xA2, 0xDA),
            Colour::OpenSuse => (0x73, 0xBA, 0x25),
            Colour::Arch => (0x17, 0x93, 0xD1),
            Colour::Manjaro => (0x35, 0xBF, 0xA4),
            Colour::Mint => (0x87, 0xCF, 0x3E),
        }
    }

    /// The dotted lines: a darker shade of the paper.
    pub fn line(self) -> (u8, u8, u8) {
        let (r, g, b) = self.paper();
        let shade = |c: u8| (f32::from(c) * LINE_SHADE).round() as u8;
        (shade(r), shade(g), shade(b))
    }

    /// The note's text: dark or white, whichever contrasts more with the paper.
    pub fn text(self) -> (u8, u8, u8) {
        let paper = self.paper();
        if contrast_ratio(DARK_TEXT, paper) >= contrast_ratio(LIGHT_TEXT, paper) {
            DARK_TEXT
        } else {
            LIGHT_TEXT
        }
    }
}

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
        for colour in Colour::ALL {
            let ratio = contrast_ratio(colour.text(), colour.paper());
            assert!(
                ratio >= 4.5,
                "{colour:?} text has contrast {ratio:.2}, need 4.5"
            );
        }
    }

    #[test]
    fn debian_red_gets_white_text_and_yellow_gets_dark_text() {
        assert_eq!(Colour::Debian.text(), LIGHT_TEXT);
        assert_eq!(Colour::Yellow.text(), DARK_TEXT);
    }

    #[test]
    fn lines_are_a_darker_shade_of_the_paper() {
        for colour in Colour::ALL {
            assert!(relative_luminance(colour.line()) < relative_luminance(colour.paper()));
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
    fn unknown_and_old_palette_names_are_yellow() {
        for name in ["", "green", "pink", "purple", "grey", "nonsense"] {
            assert_eq!(Colour::from_name(name), Colour::Yellow);
        }
    }
}
