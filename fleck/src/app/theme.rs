//! Fleck's light/dark choice: follow COSMIC, or force light or dark in every
//! Fleck window. Remembered with cosmic-config under the app id.

use cosmic::app::Task;
use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};
use serde::{Deserialize, Serialize};

use super::Message;

const CONFIG_VERSION: u64 = 1;
const THEME_KEY: &str = "theme";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTheme {
    #[default]
    System,
    Light,
    Dark,
}

impl AppTheme {
    /// In the order the note menu lists them.
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    /// The saved choice, or `System` when nothing is saved or it can't be read.
    pub fn load(app_id: &str) -> Self {
        Config::new(app_id, CONFIG_VERSION)
            .ok()
            .and_then(|config| config.get(THEME_KEY).ok())
            .unwrap_or_default()
    }

    pub fn save(self, app_id: &str) {
        let saved =
            Config::new(app_id, CONFIG_VERSION).and_then(|config| config.set(THEME_KEY, self));
        if let Err(error) = saved {
            tracing::warn!(?error, "failed to save the theme choice");
        }
    }

    /// Switches every Fleck window to this choice. `prefer_dark` pins Light
    /// and Dark so libcosmic ignores COSMIC's own mode changes; `System`
    /// leaves it unset so they apply again.
    pub fn apply(self) -> Task<Message> {
        let theme = match self {
            Self::System => cosmic::theme::system_preference(),
            Self::Light => {
                let mut theme = cosmic::theme::system_light();
                theme.theme_type.prefer_dark(Some(false));
                theme
            }
            Self::Dark => {
                let mut theme = cosmic::theme::system_dark();
                theme.theme_type.prefer_dark(Some(true));
                theme
            }
        };
        cosmic::command::set_theme(theme)
    }

    pub fn label(self) -> String {
        match self {
            Self::System => crate::fl!("theme-system"),
            Self::Light => crate::fl!("theme-light"),
            Self::Dark => crate::fl!("theme-dark"),
        }
    }
}
