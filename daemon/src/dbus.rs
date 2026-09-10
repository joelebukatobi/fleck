//! The `io.github.joelebukatobi.Tack` D-Bus service.
//!
//! The interface implementation below holds no note state itself: every
//! method forwards a [`Request`] to the running [`crate::app::Tack`] over an
//! `mpsc` channel and, where it owes the caller an answer, awaits a paired
//! one-shot reply that `Tack::update` sends back once the change has
//! actually been applied. `NotesChanged` is only ever emitted after that
//! reply confirms the state it describes is real.
//!
//! This all runs inside zbus's own async tasks, driven by the same tokio
//! executor `cosmic::executor::Default` already owns (see the `tokio`
//! feature on the `zbus` dependency) - there is no second reactor to
//! deadlock against.

use cosmic::iced::futures::channel::{mpsc, oneshot};
use cosmic::iced::futures::SinkExt;
use uuid::Uuid;
use zbus::object_server::SignalEmitter;
use zbus::{fdo, interface};

/// Well-known bus name this service is published under.
pub const SERVICE_NAME: &str = "io.github.joelebukatobi.Tack";
/// Object path the interface is served at - the standard slash-separated
/// form of [`SERVICE_NAME`].
pub const OBJECT_PATH: &str = "/io/github/joelebukatobi/Tack";

/// One D-Bus call, carrying whatever reply channel its caller is waiting
/// on. `Tack::update` matches on this exactly like any other `Message`.
pub enum Request {
    ListNotes(oneshot::Sender<Vec<(String, String, bool)>>),
    /// `bool` reply: whether a note with that uuid existed to be shown.
    ShowNote(Uuid, oneshot::Sender<bool>),
    /// `bool` reply: whether a note with that uuid existed to be hidden.
    HideNote(Uuid, oneshot::Sender<bool>),
    /// `None` reply means note creation failed (already logged by the
    /// application) - the caller sees a D-Bus error rather than a bogus id.
    NewNote(oneshot::Sender<Option<Uuid>>),
    /// `bool` reply: whether a note with that uuid existed to be deleted.
    DeleteNote(Uuid, oneshot::Sender<bool>),
    /// `bool` reply: whether notes are now visible (the rule from
    /// `next_all_visible`).
    ToggleAll(oneshot::Sender<bool>),
    /// Opens the notes list window, or raises it if already open. This is
    /// how a second `tack` launch (which loses the D-Bus name race and
    /// exits immediately - see `main.rs`) gets the user back to the list.
    ShowList(oneshot::Sender<()>),
    Quit,
}

/// The D-Bus object. Cheap to construct - it's just a sender.
pub struct TackInterface {
    requests: mpsc::Sender<Request>,
}

impl TackInterface {
    pub fn new(requests: mpsc::Sender<Request>) -> Self {
        Self { requests }
    }
}

/// The application-side channel closed, which only happens while the whole
/// process is shutting down. There's no meaningful reply to give a D-Bus
/// caller at that point beyond "the service is going away".
fn dead_app() -> fdo::Error {
    fdo::Error::Failed("tack: the application isn't responding".to_string())
}

fn parse_uuid(raw: &str) -> fdo::Result<Uuid> {
    Uuid::parse_str(raw).map_err(|e| fdo::Error::InvalidArgs(format!("not a uuid: {e}")))
}

fn unknown_note(uuid: Uuid) -> fdo::Error {
    fdo::Error::Failed(format!("no such note: {uuid}"))
}

#[interface(name = "io.github.joelebukatobi.Tack")]
impl TackInterface {
    async fn list_notes(&self) -> fdo::Result<Vec<(String, String, bool)>> {
        let (tx, rx) = oneshot::channel();
        self.requests.clone().send(Request::ListNotes(tx)).await.map_err(|_| dead_app())?;
        rx.await.map_err(|_| dead_app())
    }

    async fn show_note(
        &self,
        uuid: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let uuid = parse_uuid(uuid)?;
        let (tx, rx) = oneshot::channel();
        self.requests.clone().send(Request::ShowNote(uuid, tx)).await.map_err(|_| dead_app())?;
        if rx.await.map_err(|_| dead_app())? {
            let _ = emitter.notes_changed().await;
            Ok(())
        } else {
            Err(unknown_note(uuid))
        }
    }

    async fn hide_note(
        &self,
        uuid: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let uuid = parse_uuid(uuid)?;
        let (tx, rx) = oneshot::channel();
        self.requests.clone().send(Request::HideNote(uuid, tx)).await.map_err(|_| dead_app())?;
        if rx.await.map_err(|_| dead_app())? {
            let _ = emitter.notes_changed().await;
            Ok(())
        } else {
            Err(unknown_note(uuid))
        }
    }

    async fn new_note(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<String> {
        let (tx, rx) = oneshot::channel();
        self.requests.clone().send(Request::NewNote(tx)).await.map_err(|_| dead_app())?;
        match rx.await.map_err(|_| dead_app())? {
            Some(uuid) => {
                let _ = emitter.notes_changed().await;
                Ok(uuid.to_string())
            }
            None => Err(fdo::Error::Failed("tack: failed to create note".to_string())),
        }
    }

    async fn delete_note(
        &self,
        uuid: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let uuid = parse_uuid(uuid)?;
        let (tx, rx) = oneshot::channel();
        self.requests.clone().send(Request::DeleteNote(uuid, tx)).await.map_err(|_| dead_app())?;
        if rx.await.map_err(|_| dead_app())? {
            let _ = emitter.notes_changed().await;
            Ok(())
        } else {
            Err(unknown_note(uuid))
        }
    }

    async fn toggle_all(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.requests.clone().send(Request::ToggleAll(tx)).await.map_err(|_| dead_app())?;
        let visible = rx.await.map_err(|_| dead_app())?;
        let _ = emitter.notes_changed().await;
        Ok(visible)
    }

    async fn show_list(&self) -> fdo::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.requests.clone().send(Request::ShowList(tx)).await.map_err(|_| dead_app())?;
        rx.await.map_err(|_| dead_app())
    }

    async fn quit(&self) -> fdo::Result<()> {
        self.requests.clone().send(Request::Quit).await.map_err(|_| dead_app())
    }

    #[zbus(signal)]
    async fn notes_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
