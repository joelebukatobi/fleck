mod app;
// Retained for upcoming design work (the WCAG contrast checker will be
// wanted then); nothing renders from it right now, which leaves several of
// its pub items unused.
#[allow(dead_code)]
mod palette;

use sticky_notes_core::Store;

/// The current UTC time as an RFC 3339 timestamp, e.g.
/// `2026-09-04T10:15:00Z`. `core` has no clock of its own — the daemon is
/// the one caller allowed to supply "now" to `Store::create`.
///
/// Implemented against `std::time::SystemTime` directly (no `chrono`/`time`
/// dependency) using the days-since-epoch civil calendar conversion from
/// Howard Hinnant's `chrono-Compatible Low-Level Date Algorithms`.
pub fn now_rfc3339() -> String {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    rfc3339_from_unix_secs(since_epoch.as_secs())
}

/// Pure conversion of a Unix timestamp (seconds since 1970-01-01T00:00:00Z)
/// to an RFC 3339 UTC string, e.g. `2026-09-04T10:15:00Z`.
///
/// Implemented against plain integer arithmetic (no `chrono`/`time`
/// dependency) using the days-since-epoch civil calendar conversion from
/// Howard Hinnant's `chrono-Compatible Low-Level Date Algorithms`.
fn rfc3339_from_unix_secs(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let time_of_day = secs % 86_400;
    let (hour, minute, second) = (time_of_day / 3600, (time_of_day / 60) % 60, time_of_day % 60);

    // civil_from_days: days since 1970-01-01 -> (year, month, day).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if month <= 2 { year + 1 } else { year };

    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values verified with `date -u -d @<secs> +%Y-%m-%dT%H:%M:%SZ`.
    #[test]
    fn rfc3339_from_unix_secs_matches_known_dates() {
        assert_eq!(rfc3339_from_unix_secs(0), "1970-01-01T00:00:00Z");
        assert_eq!(
            rfc3339_from_unix_secs(951_782_400),
            "2000-02-29T00:00:00Z",
            "leap year, century divisible by 400"
        );
        assert_eq!(
            rfc3339_from_unix_secs(4_107_542_400),
            "2100-03-01T00:00:00Z",
            "century NOT a leap year"
        );
        assert_eq!(
            rfc3339_from_unix_secs(1_704_067_199),
            "2023-12-31T23:59:59Z",
            "year boundary"
        );
        assert_eq!(rfc3339_from_unix_secs(1_709_164_800), "2024-02-29T00:00:00Z");
    }
}

fn notes_dir() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("HOME is set");
            std::path::PathBuf::from(home).join(".local/share")
        });
    base.join("tack/notes")
}

fn main() -> cosmic::iced::Result {
    let store = Store::new(notes_dir());

    // Deliberately temporary: the simplest thing that gets a note file on
    // disk so the GUI has something to open. Task 13 replaces this with a
    // D-Bus call to a running instance; this is a one-shot, no-GUI path.
    if std::env::args().any(|arg| arg == "--new-note") {
        return match store.create(&now_rfc3339(), palette::Colour::Yellow.name()) {
            Ok(note) => {
                println!("{}", note.frontmatter.uuid);
                Ok(())
            }
            Err(e) => {
                eprintln!("tack: failed to create note: {e}");
                std::process::exit(1);
            }
        };
    }

    // `exit_on_close(false)`: without it, libcosmic force-exits the whole
    // app the instant the *main* window closes, even if other note windows
    // are still open (`Core::exit_on_main_window_closed`, on by default).
    // The main window is just the first note here, not special - `Tack`
    // itself decides when to exit, in `Message::NoteClosed`, once its
    // `windows` map is empty (i.e. the *last* note window closed).
    cosmic::app::run::<app::Tack>(
        cosmic::app::Settings::default().exit_on_close(false),
        store,
    )
}
