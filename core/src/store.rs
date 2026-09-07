use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::note::{parse, Note, ParseError};

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, id: Uuid) -> PathBuf {
        self.dir.join(format!("{id}.md"))
    }

    pub fn list(&self) -> std::io::Result<Vec<Result<Note, ParseError>>> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };

        let mut out = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            out.push(parse(&std::fs::read_to_string(&path)?));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::{serialize, Frontmatter, Note};

    fn write_note(dir: &std::path::Path, id: Uuid, body: &str) {
        let note = Note {
            frontmatter: Frontmatter {
                version: 1,
                uuid: id,
                created: "2026-09-04T10:15:00Z".into(),
                color: "yellow".into(),
            },
            body: body.into(),
        };
        std::fs::write(dir.join(format!("{id}.md")), serialize(&note)).unwrap();
    }

    #[test]
    fn lists_notes_in_directory() {
        let dir = tempfile::tempdir().unwrap();
        write_note(dir.path(), Uuid::from_u128(1), "one\n");
        write_note(dir.path(), Uuid::from_u128(2), "two\n");
        let store = Store::new(dir.path());
        assert_eq!(store.list().unwrap().len(), 2);
    }

    #[test]
    fn corrupt_note_does_not_hide_the_others() {
        let dir = tempfile::tempdir().unwrap();
        write_note(dir.path(), Uuid::from_u128(1), "fine\n");
        std::fs::write(dir.path().join("broken.md"), "no fence here").unwrap();
        let results = Store::new(dir.path()).list().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    }

    #[test]
    fn ignores_backup_and_non_markdown_files() {
        let dir = tempfile::tempdir().unwrap();
        write_note(dir.path(), Uuid::from_u128(1), "fine\n");
        std::fs::write(dir.path().join("00000000-0000-0000-0000-000000000001.md.bak"), "x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        assert_eq!(Store::new(dir.path()).list().unwrap().len(), 1);
    }

    #[test]
    fn missing_directory_lists_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("does-not-exist"));
        assert!(store.list().unwrap().is_empty());
    }
}
