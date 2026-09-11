//! `cosmic-ext-applet-fleck`: a single icon in the COSMIC panel. Clicking it
//! asks the running `fleck` instance (`io.github.joelebukatobi.Fleck` on the
//! session bus) to show its notes list window; if nothing owns that name,
//! it starts `fleck` instead. No popup, no menu - see docs/ux.md "Panel
//! applet".

use std::path::{Path, PathBuf};
use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::Element;

mod icons;

/// Separate from Fleck's own id (`io.github.joelebukatobi.Fleck`) - this is a
/// different program.
const APP_ID: &str = "io.github.joelebukatobi.FleckApplet";

/// Mirrors `daemon/src/main.rs`'s `RELAUNCH_TIMEOUT`: long enough for a
/// normal D-Bus round trip, short enough that a wedged Fleck doesn't freeze
/// the panel.
const CLICK_TIMEOUT: Duration = Duration::from_secs(3);

/// Client-side view of the `io.github.joelebukatobi.Fleck` service. Mirrors
/// the (private) `Fleck` proxy trait in `daemon/src/main.rs`, trimmed to the
/// one method this applet calls - not shared from `daemon` because the
/// daemon's copy isn't `pub` and a four-line proxy trait isn't worth
/// threading a new public export through `daemon`/`core` for.
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

/// The sibling `fleck` binary's path, computed from the applet's own path -
/// in development both land in `target/release/`, packaged both land in
/// `/usr/bin`, so this works in either case without depending on `$PATH`.
fn sibling_binary_path(applet_exe: &Path, name: &str) -> PathBuf {
    match applet_exe.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

/// Spawns `fleck` detached (never awaited) next to this binary. Logs and
/// gives up on any failure rather than crashing the applet.
fn spawn_fleck() {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("cosmic-ext-applet-fleck: couldn't determine own path: {e}");
            return;
        }
    };
    let fleck = sibling_binary_path(&exe, "fleck");
    if let Err(e) = std::process::Command::new(&fleck).spawn() {
        eprintln!("cosmic-ext-applet-fleck: failed to start {}: {e}", fleck.display());
    }
}

async fn handle_click() {
    match try_show_list().await {
        ShowListOutcome::Shown => {}
        ShowListOutcome::NotRunning => spawn_fleck(),
        ShowListOutcome::Failed(msg) => {
            eprintln!("cosmic-ext-applet-fleck: ShowList failed: {msg}");
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
            // the same tokio-backed one the daemon uses - zbus's `tokio`
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

fn main() -> cosmic::iced::Result {
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

    #[test]
    fn sibling_path_is_computed_from_applets_own_directory() {
        assert_eq!(
            sibling_binary_path(Path::new("/usr/bin/cosmic-ext-applet-fleck"), "fleck"),
            PathBuf::from("/usr/bin/fleck")
        );
        assert_eq!(
            sibling_binary_path(
                Path::new("/home/user/project/target/release/cosmic-ext-applet-fleck"),
                "fleck"
            ),
            PathBuf::from("/home/user/project/target/release/fleck")
        );
    }

    #[test]
    fn sibling_path_falls_back_to_bare_name_with_no_parent() {
        assert_eq!(sibling_binary_path(Path::new("cosmic-ext-applet-fleck"), "fleck"), PathBuf::from("fleck"));
    }
}
