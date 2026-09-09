mod app;
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
    let secs = since_epoch.as_secs();
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
    cosmic::app::run::<app::Tack>(
        cosmic::app::Settings::default().no_main_window(true),
        store,
    )
}
