//! One-time rename of the pre-rename (Tack) XDG directories to their Fleck
//! equivalents, run at startup before any notes or window state are loaded.

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
