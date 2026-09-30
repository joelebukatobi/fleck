mod app;
mod applet;
mod dbus;
mod i18n;
mod icons;
mod notify;
mod palette;
mod ruled;
mod undo;

use std::time::Duration;

use cosmic::iced::futures::channel::mpsc;
use fleck_core::{Store, WindowState};

/// How long a losing-the-name-race relaunch waits for the running instance
/// to answer `ShowList()` before giving up. Long enough for a normal
/// D-Bus round trip, short enough that a wedged running instance doesn't
/// hang every subsequent `fleck` invocation forever.
const RELAUNCH_TIMEOUT: Duration = Duration::from_secs(3);

/// Client-side view of the `io.github.joelebukatobi.Fleck` service that
/// `dbus.rs` implements - one trait method per D-Bus method, generated into
/// an async `FleckProxy` by the `#[zbus::proxy]` macro.
#[zbus::proxy(
    default_service = "io.github.joelebukatobi.Fleck",
    default_path = "/io/github/joelebukatobi/Fleck",
    interface = "io.github.joelebukatobi.Fleck"
)]
trait Fleck {
    fn list_notes(&self) -> zbus::Result<Vec<(String, String, bool)>>;
    fn show_note(&self, uuid: &str) -> zbus::Result<()>;
    fn hide_note(&self, uuid: &str) -> zbus::Result<()>;
    fn new_note(&self) -> zbus::Result<String>;
    fn delete_note(&self, uuid: &str) -> zbus::Result<()>;
    fn toggle_all(&self) -> zbus::Result<bool>;
    fn show_list(&self) -> zbus::Result<()>;
    fn quit(&self) -> zbus::Result<()>;
}

/// Connects to the session bus and builds a proxy for a running `fleck`
/// instance. Errors here (no session bus, or nothing owns the well-known
/// name yet) all mean the same thing to a caller: there's no daemon to talk
/// to right now.
async fn connect() -> zbus::Result<FleckProxy<'static>> {
    let connection = zbus::Connection::session().await?;
    FleckProxy::new(&connection).await
}

/// Every CLI flag prints its own diagnostics and reports success/failure
/// only through this exit code - `main` never prints on their behalf.
const OK: i32 = 0;
const FAILED: i32 = 1;

async fn cli_list() -> i32 {
    match connect().await {
        Ok(proxy) => match proxy.list_notes().await {
            Ok(notes) => {
                for (uuid, name, visible) in notes {
                    println!(
                        "{uuid}\t{name}\t{}",
                        if visible { "visible" } else { "hidden" }
                    );
                }
                OK
            }
            Err(e) => {
                eprintln!("fleck: --list failed: {e}");
                FAILED
            }
        },
        Err(e) => {
            eprintln!("fleck: no running daemon to list notes from: {e}");
            FAILED
        }
    }
}

/// `--new-note` predates the daemon's D-Bus service (Task 11): it used to
/// create a note file directly. Now it prefers asking a running daemon (so
/// the new note actually gets a window), and only falls back to the old
/// direct-file-creation path when nothing is listening.
async fn cli_new_note(store: &Store) -> i32 {
    match connect().await {
        Ok(proxy) => match proxy.new_note().await {
            Ok(uuid) => {
                println!("{uuid}");
                OK
            }
            Err(e) => {
                eprintln!("fleck: --new-note failed: {e}");
                FAILED
            }
        },
        Err(_) => match store.create(&now_rfc3339(), palette::Colour::Default.name()) {
            Ok(note) => {
                println!("{}", note.frontmatter.uuid);
                OK
            }
            Err(e) => {
                eprintln!("fleck: failed to create note: {e}");
                FAILED
            }
        },
    }
}

async fn cli_show(uuid: &str) -> i32 {
    match connect().await {
        Ok(proxy) => match proxy.show_note(uuid).await {
            Ok(()) => OK,
            Err(e) => {
                eprintln!("fleck: --show failed: {e}");
                FAILED
            }
        },
        Err(e) => {
            eprintln!("fleck: no running daemon: {e}");
            FAILED
        }
    }
}

async fn cli_hide(uuid: &str) -> i32 {
    match connect().await {
        Ok(proxy) => match proxy.hide_note(uuid).await {
            Ok(()) => OK,
            Err(e) => {
                eprintln!("fleck: --hide failed: {e}");
                FAILED
            }
        },
        Err(e) => {
            eprintln!("fleck: no running daemon: {e}");
            FAILED
        }
    }
}

async fn cli_delete(uuid: &str) -> i32 {
    match connect().await {
        Ok(proxy) => match proxy.delete_note(uuid).await {
            Ok(()) => OK,
            Err(e) => {
                eprintln!("fleck: --delete failed: {e}");
                FAILED
            }
        },
        Err(e) => {
            eprintln!("fleck: no running daemon: {e}");
            FAILED
        }
    }
}

async fn cli_toggle_all() -> i32 {
    match connect().await {
        Ok(proxy) => match proxy.toggle_all().await {
            Ok(visible) => {
                println!("{}", if visible { "visible" } else { "hidden" });
                OK
            }
            Err(e) => {
                eprintln!("fleck: --toggle-all failed: {e}");
                FAILED
            }
        },
        Err(e) => {
            eprintln!("fleck: no running daemon: {e}");
            FAILED
        }
    }
}

async fn cli_quit() -> i32 {
    match connect().await {
        Ok(proxy) => match proxy.quit().await {
            // The daemon exits as soon as it reads this call, so the
            // connection can drop out from under the reply - treat that
            // the same as success rather than reporting a spurious error.
            Ok(()) | Err(zbus::Error::InputOutput(_)) => OK,
            Err(e) => {
                eprintln!("fleck: --quit failed: {e}");
                FAILED
            }
        },
        Err(e) => {
            eprintln!("fleck: no running daemon: {e}");
            FAILED
        }
    }
}

/// Handles a recognised CLI flag and returns the process exit code, or
/// `None` if `args` names no CLI flag at all (the normal GUI startup path).
fn run_cli(args: &[String], store: &Store) -> Option<i32> {
    let flag = args.first()?.as_str();
    if !matches!(
        flag,
        "--list" | "--new-note" | "--show" | "--hide" | "--delete" | "--toggle-all" | "--quit"
    ) {
        return None;
    }
    let arg = |name: &str| {
        args.get(1).cloned().unwrap_or_else(|| {
            eprintln!("fleck: {name} requires a uuid");
            std::process::exit(FAILED);
        })
    };
    Some(zbus::block_on(async {
        match flag {
            "--list" => cli_list().await,
            "--new-note" => cli_new_note(store).await,
            "--show" => cli_show(&arg("--show")).await,
            "--hide" => cli_hide(&arg("--hide")).await,
            "--delete" => cli_delete(&arg("--delete")).await,
            "--toggle-all" => cli_toggle_all().await,
            "--quit" => cli_quit().await,
            _ => unreachable!("checked above"),
        }
    }))
}

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
    let days = (secs / 86_400).cast_signed();
    let time_of_day = secs % 86_400;
    let (hour, minute, second) = (
        time_of_day / 3600,
        (time_of_day / 60) % 60,
        time_of_day % 60,
    );

    // civil_from_days: days since 1970-01-01 -> (year, month, day).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let year = yoe.cast_signed() + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn xdg_data_home() -> std::path::PathBuf {
    std::env::var_os("XDG_DATA_HOME").map_or_else(
        || {
            let home = std::env::var_os("HOME").expect("HOME is set");
            std::path::PathBuf::from(home).join(".local/share")
        },
        std::path::PathBuf::from,
    )
}

fn xdg_state_home() -> std::path::PathBuf {
    std::env::var_os("XDG_STATE_HOME").map_or_else(
        || {
            let home = std::env::var_os("HOME").expect("HOME is set");
            std::path::PathBuf::from(home).join(".local/state")
        },
        std::path::PathBuf::from,
    )
}

/// Renames `$XDG_DATA_HOME/fleck` to `$XDG_DATA_HOME/fleck` and
/// `$XDG_STATE_HOME/fleck` to `$XDG_STATE_HOME/fleck` when a pre-rename
/// install left notes/state behind and nothing has claimed the new names
/// yet. Must run before `notes_dir()`/`window_state_path()` are used to
/// load anything, or a user's existing notes would appear to have vanished.
fn migrate_from_tack() {
    fleck_core::migrate_dir(
        &xdg_data_home().join(fleck_core::migrate::OLD_DIR_NAME),
        &xdg_data_home().join("fleck"),
    );
    fleck_core::migrate_dir(
        &xdg_state_home().join(fleck_core::migrate::OLD_DIR_NAME),
        &xdg_state_home().join("fleck"),
    );
}

fn notes_dir() -> std::path::PathBuf {
    xdg_data_home().join("fleck/notes")
}

/// `$XDG_STATE_HOME/fleck/windows.toml`, falling back to
/// `$HOME/.local/state/fleck/windows.toml`.
/// `$XDG_DATA_HOME/fleck/reminders.toml`: every reminder, in one file.
fn reminders_path() -> std::path::PathBuf {
    xdg_data_home().join("fleck/reminders.toml")
}

fn window_state_path() -> std::path::PathBuf {
    xdg_state_home().join("fleck/windows.toml")
}

/// Every note window - the main one included - opens at this size unless
/// resized before (see `app::window_size_for`).
const DEFAULT_WINDOW_SIZE: cosmic::iced::Size = cosmic::iced::Size::new(512.0, 768.0);

/// Logs go to stderr. `RUST_LOG` overrides the default level, e.g.
/// `RUST_LOG=fleck=debug fleck`.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,fleck=info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn main() -> cosmic::iced::Result {
    init_tracing();
    i18n::init();

    // `--applet` dispatches to the panel icon and nothing else: no
    // data-directory migration, no D-Bus name acquisition, no note store,
    // no window state. Checked before any of that runs.
    if std::env::args().nth(1).as_deref() == Some("--applet") {
        return applet::run();
    }

    // `--background` starts Fleck with no windows: it waits to fire reminders,
    // and shows itself when the panel icon or a command asks it to.
    let background = std::env::args().any(|arg| arg == "--background");

    // Must run before anything below reads notes_dir()/window_state_path(),
    // so a pre-rename (Fleck) install's notes and window state are in place
    // by the time they're loaded.
    migrate_from_tack();

    let store = Store::new(notes_dir());

    // `--list`/`--show`/etc: connect to a running daemon, make the one
    // call, print its result, and exit - no GUI involved. This runs before
    // `cosmic::app::run` so there is no ambient async executor yet; each
    // call gets its own short-lived one via `zbus::block_on`.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = run_cli(&args, &store) {
        std::process::exit(code);
    }

    let state_path = window_state_path();
    let window_state = match WindowState::load(&state_path) {
        Ok(state) => state,
        Err(e) => {
            tracing::warn!("failed to read window state {}: {e}", state_path.display());
            WindowState::default()
        }
    };

    // Acquire the well-known D-Bus name *before* any window opens - not
    // inside the app's `Subscription` (as it used to be), which only ran
    // after `init()` had already opened windows. Racing two processes both
    // past `init()` meant a second launch could open its own editable
    // windows before ever learning it lost the name, and two processes
    // could then save the same note file. Doing it here, in the same
    // pre-GUI `zbus::block_on` path the CLI flags above already use, means
    // this process knows whether it owns the name before it loads a single
    // note or opens a single window.
    //
    // `zbus::block_on` (with the crate's `tokio` feature) runs on a
    // `OnceLock`-cached, process-lifetime multi-threaded runtime - it is
    // not torn down when this call returns, so the connection built here
    // (and its D-Bus service tasks) keeps running for the life of the
    // process once handed to the app below.
    let (dbus_tx, dbus_rx) = mpsc::channel::<dbus::Request>(16);
    let connection = zbus::block_on(async {
        zbus::connection::Builder::session()?
            .serve_at(dbus::OBJECT_PATH, dbus::FleckInterface::new(dbus_tx))?
            .name(dbus::SERVICE_NAME)?
            .build()
            .await
    });

    let connection = match connection {
        Ok(connection) => connection,
        Err(e) => {
            // Another instance already owns the name (or the session bus
            // isn't reachable at all). Either way, this process has no
            // business loading notes or opening windows - ask whoever does
            // own it to show the list, then get out of the way.
            tracing::info!("{} is already owned: {e}", dbus::SERVICE_NAME);
            // Bounded: a running instance that's wedged (compositor stuck,
            // deadlocked, whatever) must not hang this process - and every
            // future relaunch - forever waiting on a reply that never
            // comes. `zbus::block_on` already runs on the process-lifetime
            // tokio runtime (see the name-acquisition comment above), so
            // `tokio::time::timeout` bounds it without a second runtime.
            let code = zbus::block_on(async {
                match tokio::time::timeout(RELAUNCH_TIMEOUT, async {
                    let proxy = connect().await?;
                    proxy.show_list().await
                })
                .await
                {
                    Ok(Ok(())) => OK,
                    Ok(Err(e)) => {
                        tracing::error!("failed to ask the running instance to show the list: {e}");
                        FAILED
                    }
                    Err(_) => {
                        tracing::error!(
                            "the running instance did not respond within {}s - it may be stuck",
                            RELAUNCH_TIMEOUT.as_secs()
                        );
                        FAILED
                    }
                }
            });
            std::process::exit(code);
        }
    };

    // `exit_on_close(false)`: without it, libcosmic force-exits the whole
    // app the instant the *main* window closes, even if other note windows
    // are still open (`Core::exit_on_main_window_closed`, on by default).
    // The main window is the notes list, not special beyond being first -
    // `Fleck` itself decides when to exit, once every window (list and
    // notes) is gone.
    cosmic::app::run::<app::Fleck>(
        cosmic::app::Settings::default()
            .exit_on_close(false)
            .size(DEFAULT_WINDOW_SIZE),
        app::Flags {
            store,
            window_state,
            state_path,
            reminders_path: reminders_path(),
            background,
            dbus_connection: connection,
            dbus_rx,
        },
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
        assert_eq!(
            rfc3339_from_unix_secs(1_709_164_800),
            "2024-02-29T00:00:00Z"
        );
    }
}
