use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    pub version: u32,
    pub uuid: Uuid,
    pub created: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub frontmatter: Frontmatter,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParseError {
    #[error("missing +++ frontmatter fence")]
    MissingFence,
    #[error("invalid frontmatter: {0}")]
    InvalidFrontmatter(String),
}

/// The note format version this build writes.
pub const FORMAT_VERSION: u32 = 1;

pub fn parse(text: &str) -> Result<Note, ParseError> {
    let rest = text.strip_prefix("+++\n").ok_or(ParseError::MissingFence)?;
    let end = rest.find("\n+++\n").ok_or(ParseError::MissingFence)?;
    let (fm_text, after) = rest.split_at(end);
    let body = after.strip_prefix("\n+++\n").unwrap_or("");
    let frontmatter: Frontmatter =
        toml::from_str(fm_text).map_err(|e| ParseError::InvalidFrontmatter(e.to_string()))?;
    Ok(Note { frontmatter, body: unescape_fences(body) })
}
/// A body line of exactly `+++` would otherwise read back as the frontmatter
/// fence, truncating the note. Escape it on write, undo it on read.
fn escape_fences(body: &str) -> String {
    body.replace("\n+++\n", "\n\\+++\n")
}

fn unescape_fences(body: &str) -> String {
    body.replace("\n\\+++\n", "\n+++\n")
}

pub fn serialize(note: &Note) -> String {
    let fm = toml::to_string(&note.frontmatter).expect("frontmatter is serializable");
    format!("+++\n{fm}+++\n{}", escape_fences(&note.body))
}

pub fn title(note: &Note) -> &str {
    note.body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "+++\nversion = 1\nuuid = \"3f2a1c88-0000-4000-8000-000000000000\"\ncreated = \"2026-09-04T10:15:00Z\"\ncolor = \"yellow\"\n+++\nGroceries\n\nback door key under mat\n";

    #[test]
    fn parses_frontmatter_and_body() {
        let note = parse(SAMPLE).expect("parses");
        assert_eq!(note.frontmatter.version, 1);
        assert_eq!(note.frontmatter.color, "yellow");
        assert_eq!(note.body, "Groceries\n\nback door key under mat\n");
    }

    #[test]
    fn rejects_missing_fence() {
        assert_eq!(parse("just text"), Err(ParseError::MissingFence));
    }

    #[test]
    fn rejects_bad_frontmatter() {
        let bad = "+++\nnot valid toml =\n+++\nbody\n";
        assert!(matches!(parse(bad), Err(ParseError::InvalidFrontmatter(_))));
    }

    #[test]
    fn empty_body_is_allowed() {
        let text = "+++\nversion = 1\nuuid = \"3f2a1c88-0000-4000-8000-000000000000\"\ncreated = \"2026-09-04T10:15:00Z\"\ncolor = \"yellow\"\n+++\n";
        assert_eq!(parse(text).expect("parses").body, "");
    }

    fn sample_note(body: &str) -> Note {
        Note {
            frontmatter: Frontmatter {
                version: 1,
                uuid: Uuid::nil(),
                created: "2026-09-04T10:15:00Z".into(),
                color: "yellow".into(),
            },
            body: body.to_string(),
        }
    }

    #[test]
    fn round_trips_a_simple_note() {
        let note = sample_note("Groceries\n\nmilk\n");
        assert_eq!(parse(&serialize(&note)).expect("parses"), note);
    }

    #[test]
    fn title_is_first_non_empty_line() {
        assert_eq!(title(&sample_note("\n\nGroceries\nmilk\n")), "Groceries");
        assert_eq!(title(&sample_note("")), "");
    }

    proptest::proptest! {
        #[test]
        fn round_trips_arbitrary_bodies(body in "[^\u{0}]{0,400}") {
            let note = sample_note(&body);
            let reparsed = parse(&serialize(&note)).expect("parses");
            proptest::prop_assert_eq!(reparsed.body, note.body);
        }
    }
}
