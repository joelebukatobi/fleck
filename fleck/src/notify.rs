//! Desktop notifications, for reminders coming due. Uses the freedesktop
//! notification service over D-Bus, which COSMIC provides - the same one every
//! other desktop app talks to, so notifications land in COSMIC's own tray and
//! follow its do-not-disturb setting.

use std::collections::HashMap;

/// A reminder's notification stays until it is dismissed: 0 means "never
/// expire". A reminder that flashed past unseen has failed at its one job.
const NEVER_EXPIRE: i32 = 0;
/// Critical urgency, so the desktop shows it as a banner rather than filing it
/// straight into the tray - the difference between being reminded and finding
/// out later.
const URGENCY_CRITICAL: u8 = 2;
/// The freedesktop sound for an alarm going off. COSMIC's notification service
/// advertises the `sound` capability and plays it.
const ALARM_SOUND: &str = "alarm-clock-elapsed";

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

/// Shows a notification for a reminder: a banner that stays, with the alarm
/// sound. Failure is logged, not retried: a missed notification must never
/// take Fleck down with it, and the reminder is still in the list.
pub async fn send(summary: String, body: String) {
    let sent = async {
        let connection = zbus::Connection::session().await?;
        let proxy = NotificationsProxy::new(&connection).await?;
        let hints = HashMap::from([
            ("urgency", zbus::zvariant::Value::U8(URGENCY_CRITICAL)),
            ("sound-name", zbus::zvariant::Value::from(ALARM_SOUND)),
            ("category", zbus::zvariant::Value::from("x-fleck.reminder")),
        ]);
        proxy
            .notify(
                "Fleck",
                0,
                crate::app::FLECK_ICON,
                &summary,
                &body,
                &[],
                hints,
                NEVER_EXPIRE,
            )
            .await
    }
    .await;
    if let Err(error) = sent {
        tracing::warn!(?error, "failed to show a notification");
    }
}
