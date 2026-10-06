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
/// hears is a reminder missed.
///
/// Fleck's own sound, built into the binary so it is there however Fleck was
/// installed - including inside a Flatpak sandbox, which has no sound theme and
/// no player to shell out to. Kept as a WAV rather than the mp3 it was
/// delivered as, so playing it needs no decoder.
const REMINDER_SOUND: &[u8] = include_bytes!("../../data/sounds/reminder.wav");
/// How long the alarm keeps sounding, repeating the clip.
const ALARM_LENGTH: std::time::Duration = std::time::Duration::from_secs(10);

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

/// Plays the alarm for `ALARM_LENGTH`, repeating the sound - one short chime is
/// easy to miss across a room. Runs on its own thread, so neither the repeats
/// nor a sound server that never answers hold up the UI.
pub fn play_alarm() {
    let (channels, rate, pcm) = match crate::audio::wav_pcm(REMINDER_SOUND) {
        Ok(sound) => sound,
        Err(error) => {
            tracing::debug!(%error, "the built-in alarm sound is unreadable");
            return;
        }
    };
    std::thread::spawn(move || {
        let until = std::time::Instant::now() + ALARM_LENGTH;
        while std::time::Instant::now() < until {
            if let Err(error) = crate::audio::play(pcm, rate, channels) {
                tracing::debug!(%error, "the reminder is silent");
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_alarm_sound_is_playable() {
        let (channels, rate, pcm) =
            crate::audio::wav_pcm(REMINDER_SOUND).expect("the bundled sound is a WAV file");

        assert_eq!(channels, 1, "mono, so it plays on any output");
        assert_eq!(rate, 22_050);
        assert!(!pcm.is_empty());
        // Two bytes a sample: the clip should be about a second long, which is
        // what makes repeating it for ten seconds sound like an alarm.
        let seconds = pcm.len() as f32 / 2.0 / rate as f32;
        assert!((0.5..3.0).contains(&seconds), "{seconds} seconds");
    }
}
