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
/// The freedesktop sound for an alarm going off, asked for by name in the
/// notification's hints.
const ALARM_SOUND: &str = "alarm-clock-elapsed";
/// ...and played by Fleck itself as well: COSMIC's notification service
/// advertises the `sound` capability but stays silent, and a reminder nobody
/// hears is a reminder missed. The sound is part of the freedesktop sound
/// theme, which every desktop ships.
const ALARM_SOUND_FILES: [&str; 2] = [
    "/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga",
    "/usr/share/sounds/freedesktop/stereo/complete.oga",
];
/// Players to try, in order. Both come with PipeWire and PulseAudio, so one of
/// them is present on a COSMIC desktop; if neither is, the reminder is still
/// shown, just silently.
const PLAYERS: [&str; 2] = ["pw-play", "paplay"];

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

/// Plays the alarm sound, if a player and a sound file can be found. Detached
/// and reaped on its own thread, so a stuck player can't hold up the UI.
pub fn play_alarm() {
    let Some(sound) = ALARM_SOUND_FILES
        .iter()
        .find(|path| std::path::Path::new(path).is_file())
    else {
        tracing::debug!("no alarm sound file found");
        return;
    };
    for player in PLAYERS {
        match std::process::Command::new(player).arg(sound).spawn() {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return;
            }
            Err(error) => tracing::debug!(?error, player, "player not available"),
        }
    }
    tracing::debug!("no audio player found; the reminder is silent");
}
