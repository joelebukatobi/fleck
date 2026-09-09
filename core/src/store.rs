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
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            // Use the entry's file type (not following symlinks) so a
            // directory named "<uuid>.md" is skipped rather than reported
            // as an unreadable note.
            match entry.file_type() {
                Ok(ft) if ft.is_file() => {}
                _ => continue,
            }
            out.push(match std::fs::read_to_string(&path) {
                Ok(text) => parse(&text),
                Err(e) => Err(ParseError::Unreadable(e.to_string())),
            });
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

    pub fn create(&self, now: &str, color: &str) -> std::io::Result<Note> {
        let note = Note {
            frontmatter: crate::note::Frontmatter {
                version: crate::note::FORMAT_VERSION,
                uuid: Uuid::new_v4(),
                created: now.to_string(),
                color: color.to_string(),
            },
            body: String::new(),
        };
        self.save(&note)?;
        Ok(note)
    }

    pub fn delete(&self, id: Uuid) -> std::io::Result<()> {
        for path in [self.path(id), self.backup_path(id)] {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
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

    #[test]
    fn create_writes_a_note_with_current_format_version() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let note = store.create("2026-09-07T09:00:00Z", "yellow").unwrap();
        assert_eq!(note.frontmatter.version, crate::note::FORMAT_VERSION);
        assert_eq!(note.frontmatter.created, "2026-09-07T09:00:00Z");
        assert_eq!(note.body, "");
        assert!(store.path(note.frontmatter.uuid).exists());
    }

    #[test]
    fn create_gives_each_note_a_distinct_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let a = store.create("2026-09-07T09:00:00Z", "yellow").unwrap();
        let b = store.create("2026-09-07T09:00:01Z", "yellow").unwrap();
        assert_ne!(a.frontmatter.uuid, b.frontmatter.uuid);
    }

    #[test]
    fn delete_removes_note_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let id = Uuid::from_u128(7);
        store.save(&note_with(id, "one\n")).unwrap();
        store.save(&note_with(id, "two\n")).unwrap();
        store.delete(id).unwrap();
        assert!(!store.path(id).exists());
        assert!(!store.backup_path(id).exists());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn deleting_a_missing_note_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        Store::new(dir.path()).delete(Uuid::from_u128(99)).unwrap();
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

    #[test]
    fn directory_named_like_a_note_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write_note(dir.path(), Uuid::from_u128(1), "fine\n");
        std::fs::create_dir(dir.path().join(format!("{}.md", Uuid::from_u128(2)))).unwrap();
        let results = Store::new(dir.path()).list().unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn unreadable_note_does_not_hide_the_others() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        write_note(dir.path(), Uuid::from_u128(1), "fine\n");
        let bad_path = dir.path().join(format!("{}.md", Uuid::from_u128(2)));
        write_note(dir.path(), Uuid::from_u128(2), "will be locked\n");
        std::fs::set_permissions(&bad_path, std::fs::Permissions::from_mode(0o000)).unwrap();

        let results = Store::new(dir.path()).list().unwrap();

        // Restore permissions so tempdir cleanup can remove the file.
        std::fs::set_permissions(&bad_path, std::fs::Permissions::from_mode(0o644)).unwrap();

        if std::fs::read_to_string(&bad_path).is_ok() {
            // Running as a user (e.g. root) for whom permissions aren't
            // enforced — the read succeeded despite 0o000, so skip the
            // assertion rather than failing.
            return;
        }

        assert_eq!(results.len(), 2);
        let errs: Vec<_> = results.iter().filter_map(|r| r.as_ref().err()).collect();
        assert_eq!(errs.len(), 1);
        assert!(matches!(errs[0], ParseError::Unreadable(_)));
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    }

    #[test]
    fn malformed_note_still_yields_missing_fence_error() {
        let dir = tempfile::tempdir().unwrap();
        write_note(dir.path(), Uuid::from_u128(1), "fine\n");
        std::fs::write(dir.path().join("broken2.md"), "no fence here").unwrap();
        let results = Store::new(dir.path()).list().unwrap();
        assert_eq!(results.len(), 2);
        let errs: Vec<_> = results.iter().filter_map(|r| r.as_ref().err()).collect();
        assert_eq!(errs.len(), 1);
        assert!(matches!(errs[0], ParseError::MissingFence));
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    }
}
