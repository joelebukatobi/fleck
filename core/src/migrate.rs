//! One-time moves of a user's notes into where this build of Fleck looks for
//! them, run at startup before any notes or window state are loaded: the
//! pre-rename (Tack) directories, and a packaged install's own store the first
//! time Fleck runs inside a sandbox.

use std::path::Path;

/// The old XDG directory name from before the Tack -> Fleck rename. Kept as
/// a constant, confined to this migration module, because this is the one
/// place in the app that is still allowed to know the old product name -
/// everywhere else says "Fleck".
pub const OLD_DIR_NAME: &str = "tack";

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Migrate,
    Skip,
}

/// Pure decision of whether an old directory should be renamed onto a new
/// one, given only whether each currently exists. Never merges or
/// overwrites: migrates only when the old directory exists and the new one
/// does not.
pub fn decide(old_exists: bool, new_exists: bool) -> Decision {
    if old_exists && !new_exists {
        Decision::Migrate
    } else {
        Decision::Skip
    }
}

/// Applies `decide` to a real `old`/`new` directory pair with a single
/// atomic `std::fs::rename` - nothing is copied, nothing is deleted. If the
/// rename fails (e.g. a cross-device error), `old` is left untouched, `new`
/// is not created as a side effect, and a `fleck:` message naming both
/// paths is logged; there is no fallback to copying.
pub fn migrate_dir(old: &Path, new: &Path) {
    if decide(old.exists(), new.exists()) == Decision::Migrate {
        if let Err(e) = std::fs::rename(old, new) {
            eprintln!(
                "fleck: failed to migrate {} to {} ({e}) - leaving it in place",
                old.display(),
                new.display()
            );
        }
    }
}

/// Copies `from` onto `to` the first time, leaving `from` where it is.
///
/// This is how a Flatpak picks up notes that were made by a system install:
/// the sandbox has its own store under `~/.var/app`, and the install it
/// replaces is still on disk and may still be used, so nothing is moved or
/// deleted. Runs only when there is nothing to lose - see `decide`.
pub fn import_dir(from: &Path, to: &Path) {
    // An empty store counts as no store: a sandbox is handed one by the
    // packaging before Fleck has ever written a note into it.
    if decide(from.exists(), !is_empty(to)) != Decision::Migrate {
        return;
    }
    if let Err(e) = copy_dir(from, to) {
        eprintln!(
            "fleck: failed to import {} into {} ({e}) - leaving it in place",
            from.display(),
            to.display()
        );
    }
}

/// Whether a directory is missing or holds nothing.
fn is_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none()) || !dir.exists()
}

/// Copies a directory and everything under it. Symlinks are followed as the
/// files they point at, because a note is a file wherever it lives.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_only_migrates() {
        assert_eq!(decide(true, false), Decision::Migrate);
    }

    #[test]
    fn new_only_skips() {
        assert_eq!(decide(false, true), Decision::Skip);
    }

    #[test]
    fn both_skips() {
        assert_eq!(decide(true, true), Decision::Skip);
    }

    #[test]
    fn neither_skips() {
        assert_eq!(decide(false, false), Decision::Skip);
    }

    #[test]
    fn renames_directory_and_note_stays_readable() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("tack");
        let new = tmp.path().join("fleck");
        std::fs::create_dir_all(old.join("notes")).unwrap();
        std::fs::write(old.join("notes").join("one.md"), "hello").unwrap();

        migrate_dir(&old, &new);

        assert!(!old.exists());
        assert_eq!(
            std::fs::read_to_string(new.join("notes").join("one.md")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn existing_new_dir_leaves_both_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("tack");
        let new = tmp.path().join("fleck");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("marker"), "old").unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("marker"), "new").unwrap();

        migrate_dir(&old, &new);

        assert_eq!(std::fs::read_to_string(old.join("marker")).unwrap(), "old");
        assert_eq!(std::fs::read_to_string(new.join("marker")).unwrap(), "new");
    }

    #[test]
    fn data_directory_migration_works_independently() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("data").join(OLD_DIR_NAME);
        let new = tmp.path().join("data").join("fleck");
        std::fs::create_dir_all(old.join("notes")).unwrap();

        migrate_dir(&old, &new);

        assert!(!old.exists());
        assert!(new.join("notes").is_dir());
    }

    #[test]
    fn an_import_copies_notes_and_leaves_the_original() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("share").join("fleck");
        let to = tmp.path().join("sandbox").join("fleck");
        std::fs::create_dir_all(from.join("notes").join("images")).unwrap();
        std::fs::write(from.join("notes").join("one.md"), "hello").unwrap();
        std::fs::write(from.join("notes").join("images").join("a.png"), "png").unwrap();
        std::fs::write(from.join("reminders.toml"), "reminders = []").unwrap();

        import_dir(&from, &to);

        assert_eq!(
            std::fs::read_to_string(to.join("notes").join("one.md")).unwrap(),
            "hello",
            "the notes came across"
        );
        assert_eq!(
            std::fs::read_to_string(to.join("notes").join("images").join("a.png")).unwrap(),
            "png",
            "and so did a note's images"
        );
        assert!(to.join("reminders.toml").is_file());
        assert!(
            from.join("notes").join("one.md").is_file(),
            "the install it came from still has its notes"
        );
    }

    #[test]
    fn an_import_fills_a_store_that_exists_but_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("share").join("fleck");
        let to = tmp.path().join("sandbox").join("fleck");
        std::fs::create_dir_all(from.join("notes")).unwrap();
        std::fs::write(from.join("notes").join("one.md"), "hello").unwrap();
        // What packaging leaves behind before Fleck has run: the directory,
        // and nothing in it.
        std::fs::create_dir_all(&to).unwrap();

        import_dir(&from, &to);

        assert!(to.join("notes").join("one.md").is_file());
    }

    #[test]
    fn an_import_never_touches_an_existing_store() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("share").join("fleck");
        let to = tmp.path().join("sandbox").join("fleck");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::write(from.join("marker"), "elsewhere").unwrap();
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(to.join("marker"), "mine").unwrap();

        import_dir(&from, &to);

        assert_eq!(std::fs::read_to_string(to.join("marker")).unwrap(), "mine");
    }

    #[test]
    fn state_directory_migration_works_independently() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("state").join(OLD_DIR_NAME);
        let new = tmp.path().join("state").join("fleck");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("windows.toml"), "").unwrap();

        migrate_dir(&old, &new);

        assert!(!old.exists());
        assert!(new.join("windows.toml").is_file());
    }
}
