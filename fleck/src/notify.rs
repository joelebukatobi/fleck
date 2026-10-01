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
/// installed. It is written to the cache folder the first time it is needed,
/// because players take a path, not bytes.
const REMINDER_SOUND: &[u8] = include_bytes!("../../data/sounds/reminder.mp3");
/// The freedesktop sound theme, in case the sound can't be written out.
const FALLBACK_SOUND_FILES: [&str; 2] = [
    "/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga",
    "/usr/share/sounds/freedesktop/stereo/complete.oga",
];
/// Players to try, in order. Both come with PipeWire and PulseAudio, so one of
/// them is present on a COSMIC desktop; if neither is, the reminder is still
/// shown, just silently.
const PLAYERS: [&str; 2] = ["pw-play", "paplay"];
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

/// Plays the alarm for `ALARM_SECONDS`, repeating the sound - one short
/// chime is easy to miss across a room. Runs on its own thread, so neither the
/// repeats nor a stuck player hold up the UI.
pub fn play_alarm() {
    let Some(sound) = sound_file() else {
        tracing::debug!("no alarm sound file found");
        return;
    };
    let Some(player) = PLAYERS.into_iter().find(|player| which(player)) else {
        tracing::debug!("no audio player found; the reminder is silent");
        return;
    };
    std::thread::spawn(move || {
        let until = std::time::Instant::now() + ALARM_LENGTH;
        while std::time::Instant::now() < until {
            match std::process::Command::new(player).arg(&sound).status() {
                Ok(_) => {}
                Err(error) => {
                    tracing::debug!(?error, player, "the player stopped");
                    break;
                }
            }
        }
    });
}

/// Whether `player` is on the PATH.
fn which(player: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(player).is_file()))
}

/// Fleck's sound on disk, written to the cache folder if it isn't there yet.
/// Falls back to the freedesktop sound theme if it can't be written.
fn sound_file() -> Option<std::path::PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".cache"))
        })?
        .join("fleck");
    let path = cache.join("reminder.mp3");
    // Rewritten when it is missing or a different size, so an updated sound in
    // a new version of Fleck replaces the cached copy.
    let written = std::fs::metadata(&path).map_or(0, |file| file.len());
    if written != REMINDER_SOUND.len() as u64 {
        if let Err(error) =
            std::fs::create_dir_all(&cache).and_then(|()| std::fs::write(&path, REMINDER_SOUND))
        {
            tracing::debug!(?error, "falling back to the system sound");
            return FALLBACK_SOUND_FILES
                .iter()
                .map(std::path::PathBuf::from)
                .find(|path| path.is_file());
        }
    }
    Some(path)
}
