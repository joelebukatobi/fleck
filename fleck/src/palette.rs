#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colour {
    Yellow,
    Green,
    Blue,
    Pink,
    Purple,
    Grey,
}

impl Colour {
    pub const ALL: [Colour; 6] = [
        Colour::Yellow,
        Colour::Green,
        Colour::Blue,
        Colour::Pink,
        Colour::Purple,
        Colour::Grey,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Colour::Yellow => "yellow",
            Colour::Green => "green",
            Colour::Blue => "blue",
            Colour::Pink => "pink",
            Colour::Purple => "purple",
            Colour::Grey => "grey",
        }
    }

    pub fn from_name(name: &str) -> Colour {
        Colour::ALL
            .into_iter()
            .find(|c| c.name() == name)
            .unwrap_or(Colour::Yellow)
    }

    pub fn background(self, dark: bool) -> (u8, u8, u8) {
        match (self, dark) {
            (Colour::Yellow, false) => (0xFF, 0xF3, 0xC4),
            (Colour::Yellow, true) => (0x4A, 0x3F, 0x1A),
            (Colour::Green, false) => (0xD6, 0xF5, 0xD6),
            (Colour::Green, true) => (0x1E, 0x40, 0x28),
            (Colour::Blue, false) => (0xD6, 0xE9, 0xFA),
            (Colour::Blue, true) => (0x1C, 0x35, 0x4D),
            (Colour::Pink, false) => (0xFA, 0xDC, 0xE6),
            (Colour::Pink, true) => (0x4D, 0x24, 0x33),
            (Colour::Purple, false) => (0xE6, 0xDC, 0xFA),
            (Colour::Purple, true) => (0x33, 0x2A, 0x4D),
            (Colour::Grey, false) => (0xE8, 0xE8, 0xE8),
            (Colour::Grey, true) => (0x33, 0x33, 0x33),
        }
    }

    #[allow(clippy::unused_self)] // same shape as `background`, which does use the colour
    pub fn text(self, dark: bool) -> (u8, u8, u8) {
        if dark {
            (0xF2, 0xF2, 0xF2)
        } else {
            (0x1A, 0x1A, 0x1A)
        }
    }
}

/// WCAG 2.1 relative luminance and contrast ratio.
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

pub fn contrast_ratio(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> f32 {
    let (a, b) = (relative_luminance(fg), relative_luminance(bg));
    let (lighter, darker) = if a > b { (a, b) } else { (b, a) };
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_meets_wcag_aa_in_both_themes() {
        for colour in Colour::ALL {
            for dark in [false, true] {
                let ratio = contrast_ratio(colour.text(dark), colour.background(dark));
                assert!(
                    ratio >= 4.5,
                    "{:?} in {} theme has contrast {ratio:.2}, need 4.5",
                    colour,
                    if dark { "dark" } else { "light" }
                );
            }
        }
    }

    #[test]
    fn contrast_ratio_matches_known_values() {
        // Black on white is the maximum, 21:1.
        assert!((contrast_ratio((0, 0, 0), (255, 255, 255)) - 21.0).abs() < 0.01);
        // A colour against itself is 1:1.
        assert!((contrast_ratio((120, 90, 200), (120, 90, 200)) - 1.0).abs() < 0.01);
    }

    #[test]
    fn every_colour_has_a_distinct_name() {
        let mut names: Vec<&str> = Colour::ALL.iter().map(|c| c.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "palette contains a duplicate name");
    }
}
