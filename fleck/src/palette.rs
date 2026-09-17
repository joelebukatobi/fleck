//! Note colours: classic sticky-note yellow plus colours people know from
//! Linux distributions. A note's paper and its card take a soft tint of its
//! colour in light mode and a deep shade in dark mode, so COSMIC's own text
//! colours stay readable on both.

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
}

/// How much of the colour goes into the light-mode tint; the rest is white.
const LIGHT_MIX: f32 = 0.4;
/// How much of the colour goes into the dark-mode shade; the rest is `DARK_BASE`.
const DARK_MIX: f32 = 0.3;
const DARK_BASE: (u8, u8, u8) = (0x1E, 0x1E, 0x1E);

impl Colour {
    /// In the order the colour dialog shows them. Yellow first: the default.
    pub const ALL: [Colour; 8] = [
        Colour::Yellow,
        Colour::Pop,
        Colour::Ubuntu,
        Colour::Debian,
        Colour::Fedora,
        Colour::OpenSuse,
        Colour::Arch,
        Colour::Manjaro,
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
        }
    }

    /// The recognisable colour itself: classic sticky-note yellow, or the
    /// distribution's brand colour.
    pub fn base(self) -> (u8, u8, u8) {
        match self {
            Colour::Yellow => (0xF8, 0xE4, 0x8C),
            Colour::Pop => (0x48, 0xB9, 0xC7),
            Colour::Ubuntu => (0xE9, 0x54, 0x20),
            Colour::Debian => (0xD7, 0x0A, 0x53),
            Colour::Fedora => (0x51, 0xA2, 0xDA),
            Colour::OpenSuse => (0x73, 0xBA, 0x25),
            Colour::Arch => (0x17, 0x93, 0xD1),
            Colour::Manjaro => (0x35, 0xBF, 0xA4),
        }
    }

    /// The note's paper and card colour: a soft tint in light mode, a deep
    /// shade in dark mode.
    pub fn background(self, dark: bool) -> (u8, u8, u8) {
        if dark {
            mix(self.base(), DARK_BASE, DARK_MIX)
        } else {
            mix(self.base(), (0xFF, 0xFF, 0xFF), LIGHT_MIX)
        }
    }
}

/// `amount` of `colour`, the rest `other`, per channel.
fn mix(colour: (u8, u8, u8), other: (u8, u8, u8), amount: f32) -> (u8, u8, u8) {
    let channel =
        |a: u8, b: u8| (f32::from(a) * amount + f32::from(b) * (1.0 - amount)).round() as u8;
    (
        channel(colour.0, other.0),
        channel(colour.1, other.1),
        channel(colour.2, other.2),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn contrast_ratio(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> f32 {
        let (a, b) = (relative_luminance(fg), relative_luminance(bg));
        let (lighter, darker) = if a > b { (a, b) } else { (b, a) };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn every_colour_meets_wcag_aa_with_cosmic_text_in_both_modes() {
        // Close to COSMIC's default text colours in light and dark mode.
        let (light_text, dark_text) = ((0x1A, 0x1A, 0x1A), (0xF2, 0xF2, 0xF2));
        for colour in Colour::ALL {
            for (dark, text) in [(false, light_text), (true, dark_text)] {
                let ratio = contrast_ratio(text, colour.background(dark));
                assert!(
                    ratio >= 4.5,
                    "{colour:?} in {} mode has contrast {ratio:.2}, need 4.5",
                    if dark { "dark" } else { "light" }
                );
            }
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

    #[test]
    fn light_tints_are_lighter_than_dark_shades() {
        for colour in Colour::ALL {
            assert!(
                relative_luminance(colour.background(false))
                    > relative_luminance(colour.background(true))
            );
        }
    }
}
