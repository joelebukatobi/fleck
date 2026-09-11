//! The COSMIC panel applet, run via `fleck --applet`: a single icon in the
//! panel. Clicking it asks the running `fleck` instance
//! (`io.github.joelebukatobi.Fleck` on the session bus) to show its notes
//! list window; if nothing owns that name, it starts `fleck` (itself, with
//! no `--applet` flag). No popup, no menu - see docs/ux.md "Panel applet".
//!
//! Formerly the separate `cosmic-ext-applet-fleck` binary/crate; merged into
//! `fleck` so only one binary (and one statically-linked copy of libcosmic)
//! ships. Entered from `main()` via `run()`, before any of the app's own
//! startup (data-dir migration, D-Bus name acquisition, note store, window
//! state) runs.

use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::Element;

use crate::icons;

/// Separate from Fleck's own id (`io.github.joelebukatobi.Fleck`) - this is a
/// different program, even though it now lives in the same binary.
const APP_ID: &str = "io.github.joelebukatobi.FleckApplet";

/// Mirrors `main.rs`'s `RELAUNCH_TIMEOUT`: long enough for a normal D-Bus
/// round trip, short enough that a wedged Fleck doesn't freeze the panel.
const CLICK_TIMEOUT: Duration = Duration::from_secs(3);

/// Client-side view of the `io.github.joelebukatobi.Fleck` service. Mirrors
/// the (private) `Fleck` proxy trait in `main.rs`, trimmed to the one method
/// this applet calls - not shared from there because that copy isn't `pub`
/// and a four-line proxy trait isn't worth threading a shared export through
/// for.
#[zbus::proxy(
    default_service = "io.github.joelebukatobi.Fleck",
    default_path = "/io/github/joelebukatobi/Fleck",
    interface = "io.github.joelebukatobi.Fleck"
)]
trait Fleck {
    fn show_list(&self) -> zbus::Result<()>;
}

/// What `try_show_list` learned, in a form `handle_click` can act on
/// without re-inspecting a `zbus::Error`.
enum ShowListOutcome {
    Shown,
    /// Nothing owns the well-known name - Fleck isn't running.
    NotRunning,
    /// Some other failure (session bus unreachable, call timed out, Fleck
    /// answered with an error): not our job to second-guess, just log it.
    Failed(String),
}

/// True when `err` means "nothing owns `io.github.joelebukatobi.Fleck`" -
/// the one condition that should make the applet start Fleck itself, as
/// opposed to any other failure (session bus down, call timed out, Fleck
/// replied with an error), which just gets logged.
fn is_not_running(err: &zbus::fdo::Error) -> bool {
    matches!(err, zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_))
}

async fn try_show_list() -> ShowListOutcome {
    let connection = match zbus::Connection::session().await {
        Ok(c) => c,
        Err(e) => return ShowListOutcome::Failed(e.to_string()),
    };
    let proxy = match FleckProxy::new(&connection).await {
        Ok(p) => p,
        Err(e) => return ShowListOutcome::Failed(e.to_string()),
    };
    match tokio::time::timeout(CLICK_TIMEOUT, proxy.show_list()).await {
        Ok(Ok(())) => ShowListOutcome::Shown,
        Ok(Err(e)) => {
            let fdo_err = zbus::fdo::Error::from(e);
            if is_not_running(&fdo_err) {
                ShowListOutcome::NotRunning
            } else {
                ShowListOutcome::Failed(fdo_err.to_string())
            }
        }
        Err(_) => ShowListOutcome::Failed(format!(
            "the running instance did not respond within {}s - it may be stuck",
            CLICK_TIMEOUT.as_secs()
        )),
    }
}

/// Builds the command that starts the app: this same executable
/// (`std::env::current_exe()`), with no `--applet` flag, so it runs as the
/// app rather than re-entering the applet.
fn app_command() -> std::io::Result<std::process::Command> {
    let exe = std::env::current_exe()?;
    Ok(std::process::Command::new(exe))
}

/// Spawns the app (this executable, no `--applet`) detached from the
/// applet's process group - it must keep running if the applet exits - and
/// reaps it on a background thread so it never lingers as a zombie. The
/// thread only waits on the child; it does not block the applet's UI.
fn spawn_app() {
    let mut command = match app_command() {
        Ok(command) => command,
        Err(e) => {
            tracing::error!("couldn't determine own path: {e}");
            return;
        }
    };
    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => {
            tracing::error!("failed to start the app: {e}");
        }
    }
}

async fn handle_click() {
    match try_show_list().await {
        ShowListOutcome::Shown => {}
        ShowListOutcome::NotRunning => spawn_app(),
        ShowListOutcome::Failed(msg) => {
            tracing::warn!("ShowList failed: {msg}");
        }
    }
}

pub struct FleckApplet {
    core: Core,
}

#[derive(Clone, Debug)]
pub enum Message {
    Clicked,
    ClickHandled,
}

impl cosmic::Application for FleckApplet {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        (Self { core }, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Run over the applet's own executor (cosmic::executor::Default,
            // the same tokio-backed one the app uses - zbus's `tokio`
            // feature needs a tokio reactor present) so a wedged Fleck blocks
            // only this task, never the UI thread.
            Message::Clicked => {
                Task::perform(handle_click(), |()| cosmic::Action::App(Message::ClickHandled))
            }
            Message::ClickHandled => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.core
            .applet
            .icon_button_from_handle(icons::note_pencil())
            .on_press(Message::Clicked)
            .into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

/// Entry point for `fleck --applet`, called from `main()` before anything
/// else runs.
pub fn run() -> cosmic::iced::Result {
    cosmic::applet::run::<FleckApplet>(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_unknown_means_not_running() {
        assert!(is_not_running(&zbus::fdo::Error::ServiceUnknown(
            "io.github.joelebukatobi.Fleck".to_string()
        )));
    }

    #[test]
    fn name_has_no_owner_means_not_running() {
        assert!(is_not_running(&zbus::fdo::Error::NameHasNoOwner(
            "io.github.joelebukatobi.Fleck".to_string()
        )));
    }

    #[test]
    fn other_dbus_errors_are_not_treated_as_not_running() {
        assert!(!is_not_running(&zbus::fdo::Error::Failed("fleck: broken".to_string())));
        assert!(!is_not_running(&zbus::fdo::Error::AccessDenied("nope".to_string())));
        assert!(!is_not_running(&zbus::fdo::Error::Timeout("slow".to_string())));
    }

    /// Was `sibling_path_is_computed_from_applets_own_directory` /
    /// `sibling_path_falls_back_to_bare_name_with_no_parent` when the applet
    /// spawned a sibling `fleck` binary next to itself. Now it spawns its
    /// own executable, so the thing worth asserting is that the spawn
    /// target is `current_exe()` with no arguments (in particular, no
    /// `--applet`, which would re-enter the applet instead of starting the
    /// app).
    #[test]
    fn spawn_target_is_current_exe_with_no_applet_flag() {
        let command = app_command().expect("current_exe should resolve in tests");
        assert_eq!(
            command.get_program(),
            std::env::current_exe().expect("current_exe").as_os_str()
        );
        assert_eq!(command.get_args().count(), 0, "must not pass --applet or any other flag");
    }
}
