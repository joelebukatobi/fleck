//! Translations: Fluent files under `i18n/`, embedded in the binary.
//! English is the fallback; `init` switches to the desktop's language when
//! a translation for it exists.

use std::sync::LazyLock;

use i18n_embed::fluent::{fluent_language_loader, FluentLanguageLoader};
use i18n_embed::{DefaultLocalizer, LanguageLoader, Localizer};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader = fluent_language_loader!();
    loader
        .load_fallback_language(&Localizations)
        .expect("the English strings are embedded in the binary");
    loader
});

/// Selects the desktop's language, keeping English if it has no translation.
pub fn init() {
    let localizer = DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations);
    let requested = i18n_embed::DesktopLanguageRequester::requested_languages();
    if let Err(error) = localizer.select(&requested) {
        tracing::warn!(%error, "keeping English, the locale did not load");
    }
}

/// Looks up a message in `i18n/<lang>/fleck.ftl`. Message ids are checked
/// against the English file at compile time.
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id)
    }};
    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id, $($args),*)
    }};
}
