//! Desktop notifications, for reminders coming due. Uses the freedesktop
//! notification service over D-Bus, which COSMIC provides - the same one every
//! other desktop app talks to, so notifications land in COSMIC's own tray and
//! follow its do-not-disturb setting.

use std::collections::HashMap;

/// How long COSMIC should show the notification: -1 leaves it to the desktop's
/// own setting rather than second-guessing it.
const DEFAULT_TIMEOUT: i32 = -1;

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

/// Shows a notification. Failure is logged, not retried: a missed notification
/// must never take Fleck down with it, and the reminder is still in the list.
pub async fn send(summary: String, body: String) {
    let sent = async {
        let connection = zbus::Connection::session().await?;
        let proxy = NotificationsProxy::new(&connection).await?;
        proxy
            .notify(
                "Fleck",
                0,
                crate::app::FLECK_ICON,
                &summary,
                &body,
                &[],
                HashMap::new(),
                DEFAULT_TIMEOUT,
            )
            .await
    }
    .await;
    if let Err(error) = sent {
        tracing::warn!(?error, "failed to show a notification");
    }
}
