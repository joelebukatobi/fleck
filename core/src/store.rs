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

    pub fn backup_path(&self, id: Uuid) -> PathBuf {
        self.dir.join(format!("{id}.md.bak"))
    }

    /// Write a note atomically, preserving the previous version as `.bak`.
    pub fn save(&self, note: &Note) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;

        let id = note.frontmatter.uuid;
        let target = self.path(id);

        // Copy rather than rename: a rename would leave a window in which the
        // note file does not exist.
        if target.exists() {
            std::fs::copy(&target, self.backup_path(id))?;
        }

        let temp = self.dir.join(format!(".{id}.md.tmp"));
        std::fs::write(&temp, crate::note::serialize(note))?;
        std::fs::rename(&temp, &target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::{serialize, Frontmatter, Note};

    fn note_with(id: Uuid, body: &str) -> Note {
        Note {
            frontmatter: Frontmatter {
                version: 1,
                uuid: id,
                created: "2026-09-04T10:15:00Z".into(),
                color: "yellow".into(),
            },
            body: body.into(),
        }
    }

    #[test]
    fn first_save_writes_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let id = Uuid::from_u128(7);
        store.save(&note_with(id, "first\n")).unwrap();
        assert!(store.path(id).exists());
        assert!(!store.backup_path(id).exists());
    }

    #[test]
    fn second_save_keeps_previous_version_as_backup() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let id = Uuid::from_u128(7);
        store.save(&note_with(id, "first\n")).unwrap();
        store.save(&note_with(id, "second\n")).unwrap();

        let current = std::fs::read_to_string(store.path(id)).unwrap();
        let backup = std::fs::read_to_string(store.backup_path(id)).unwrap();
        assert!(current.contains("second"));
        assert!(backup.contains("first"));
    }

    #[test]
    fn only_one_generation_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let id = Uuid::from_u128(7);
        for body in ["one\n", "two\n", "three\n"] {
            store.save(&note_with(id, body)).unwrap();
        }
        let backup = std::fs::read_to_string(store.backup_path(id)).unwrap();
        assert!(backup.contains("two"));
        assert!(!backup.contains("one"));
    }

    #[test]
    fn save_creates_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("nested/notes"));
        let id = Uuid::from_u128(7);
        store.save(&note_with(id, "hello\n")).unwrap();
        assert!(store.path(id).exists());
    }

    #[test]
    fn saved_note_reads_back_identically() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let note = note_with(Uuid::from_u128(7), "round\ntrip\n");
        store.save(&note).unwrap();
        let listed = store.list().unwrap();
        assert_eq!(listed[0].as_ref().unwrap(), &note);
    }

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
