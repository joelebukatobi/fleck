use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    pub version: u32,
    pub uuid: Uuid,
    pub created: String,
    pub color: String,
    #[serde(default)]
    pub name: String,
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
    #[error("could not read note file: {0}")]
    Unreadable(String),
    #[error("{file} holds note {uuid}, but note files must be named <uuid>.md")]
    WrongFileName { file: String, uuid: Uuid },
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
    Ok(Note {
        frontmatter,
        body: unescape_fences(body),
    })
}
/// A body line of exactly `+++` would otherwise read back as the frontmatter
/// fence, truncating the note. Escape it on write, undo it on read.
///
/// Line-wise and backslash-counting (not `str::replace` on a literal), so the
/// mapping is injective: a line of zero-or-more backslashes followed by
/// `+++` gains one leading backslash. This is invertible by `unescape_fences`
/// even when the user's own body already contains a line like `\+++`.
fn escape_fences(body: &str) -> String {
    transform_fence_lines(
        body,
        |line| line.ends_with("+++") && line[..line.len() - 3].bytes().all(|b| b == b'\\'),
        |line| format!("\\{line}"),
    )
}

/// Inverse of `escape_fences`: a line of one-or-more backslashes followed by
/// `+++` loses exactly one leading backslash.
fn unescape_fences(body: &str) -> String {
    transform_fence_lines(
        body,
        |line| {
            line.starts_with('\\')
                && line.ends_with("+++")
                && line[..line.len() - 3].bytes().all(|b| b == b'\\')
        },
        |line| line[1..].to_string(),
    )
}

fn transform_fence_lines(
    body: &str,
    matches: impl Fn(&str) -> bool,
    transform: impl Fn(&str) -> String,
) -> String {
    body.split('\n')
        .map(|line| {
            if matches(line) {
                transform(line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn serialize(note: &Note) -> String {
    let fm = toml::to_string(&note.frontmatter).expect("frontmatter is serializable");
    format!("+++\n{fm}+++\n{}", escape_fences(&note.body))
}

pub fn title(note: &Note) -> &str {
    note.body
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
}

/// Fallback display name for a note with no explicit name and no body text.
pub const UNNAMED: &str = "New note";

/// The name to show for a note: the explicit `frontmatter.name` when set,
/// otherwise the first non-empty body line (`title`), otherwise `UNNAMED`.
pub fn display_name(note: &Note) -> &str {
    if note.frontmatter.name.is_empty() {
        let title = title(note);
        if title.is_empty() {
            UNNAMED
        } else {
            title
        }
    } else {
        &note.frontmatter.name
    }
}

/// The images a note links to, as `(line index, file name)` in the order they
/// appear. A line counts when it is exactly a Markdown image link naming a
/// plain file (`![](picture.png)`, or with alt text) - the form Fleck writes
/// when an image is pasted in. Links to anything else, a URL say, are left
/// alone: they are the user's own text.
pub fn image_links(body: &str) -> Vec<(usize, String)> {
    body.lines()
        .enumerate()
        .filter_map(|(line, text)| Some((line, image_link(text.trim())?.to_string())))
        .collect()
}

/// The file name in a Markdown image link that is alone on its line.
fn image_link(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("![")?;
    let (_alt, rest) = rest.split_once("](")?;
    let name = rest.strip_suffix(')')?;
    let plain = !name.is_empty()
        && !name.contains(['/', '\\', ' ', '"'])
        && !name.contains("..")
        && name.contains('.');
    plain.then_some(name)
}

/// Whether a note is disposable: an empty or whitespace-only body AND no
/// explicit name, safe to delete instead of persisting to disk (e.g. a note
/// window closed without ever being typed into). A note with a name but no
/// body is NOT disposable — the name is content worth keeping.
pub fn is_disposable(note: &Note) -> bool {
    note.body.trim().is_empty() && note.frontmatter.name.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_links_finds_pasted_images_in_order() {
        let body = "first\n![](a.png)\nmiddle\n![shopping](b.jpg)\n";
        assert_eq!(
            image_links(body),
            vec![(1, "a.png".to_string()), (3, "b.jpg".to_string())]
        );
    }

    #[test]
    fn image_links_ignores_links_that_are_not_a_notes_own_image() {
        let body = "![](https://example.com/x.png)\n\
                    ![](../escape.png)\n\
                    ![](sub/dir.png)\n\
                    text ![](a.png) more\n\
                    ![](noextension)\n";
        assert!(image_links(body).is_empty());
    }

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
                name: String::new(),
            },
            body: body.to_string(),
        }
    }

    fn named_note(name: &str, body: &str) -> Note {
        let mut note = sample_note(body);
        note.frontmatter.name = name.to_string();
        note
    }

    #[test]
    fn display_name_prefers_explicit_name_over_body() {
        let note = named_note("Shopping List", "Groceries\nmilk\n");
        assert_eq!(display_name(&note), "Shopping List");
    }

    #[test]
    fn display_name_falls_back_to_first_body_line_when_name_empty() {
        let note = named_note("", "Groceries\nmilk\n");
        assert_eq!(display_name(&note), "Groceries");
    }

    #[test]
    fn display_name_falls_back_to_unnamed_when_name_and_body_empty() {
        let note = named_note("", "");
        assert_eq!(display_name(&note), UNNAMED);
    }

    #[test]
    fn old_note_file_without_name_key_still_parses_with_empty_name() {
        // Hand-written, not serialised: represents a note file written
        // before the `name` field existed, with no `name` key at all.
        let text = "+++\nversion = 1\nuuid = \"3f2a1c88-0000-4000-8000-000000000000\"\ncreated = \"2026-09-04T10:15:00Z\"\ncolor = \"yellow\"\n+++\nGroceries\n";
        let note = parse(text).expect("parses despite missing name key");
        assert_eq!(note.frontmatter.name, "");
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

    #[test]
    fn empty_or_whitespace_body_is_disposable() {
        assert!(is_disposable(&sample_note("")));
        assert!(is_disposable(&sample_note("   \n\t\n  ")));
    }

    #[test]
    fn body_with_content_is_not_disposable() {
        assert!(!is_disposable(&sample_note("Groceries")));
        assert!(!is_disposable(&sample_note("  x  ")));
    }

    #[test]
    fn named_note_with_empty_body_is_not_disposable() {
        assert!(!is_disposable(&named_note("Shopping List", "")));
        assert!(!is_disposable(&named_note("Shopping List", "   \n\t\n  ")));
    }

    #[test]
    fn round_trips_a_lone_fence_line() {
        let note = sample_note("+++\n");
        assert_eq!(parse(&serialize(&note)).expect("parses").body, note.body);
    }

    #[test]
    fn round_trips_a_body_with_a_bare_fence_line() {
        let note = sample_note("a\n+++\nb\n");
        assert_eq!(parse(&serialize(&note)).expect("parses").body, note.body);
    }

    #[test]
    fn round_trips_a_user_typed_escaped_fence_line() {
        let note = sample_note("a\n\\+++\nb\n");
        assert_eq!(parse(&serialize(&note)).expect("parses").body, note.body);
    }

    #[test]
    fn round_trips_a_user_typed_double_escaped_fence_line() {
        let note = sample_note("a\n\\\\+++\nb\n");
        assert_eq!(parse(&serialize(&note)).expect("parses").body, note.body);
    }

    #[test]
    fn round_trips_consecutive_fence_lines() {
        let note = sample_note("+++\n+++\n");
        assert_eq!(parse(&serialize(&note)).expect("parses").body, note.body);
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
