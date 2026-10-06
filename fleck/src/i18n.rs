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

#[cfg(test)]
mod tests {
    /// Every catalogue carries exactly the English message ids.
    ///
    /// `fl!` checks ids against English at compile time, so a translation that
    /// is missing a message falls back silently and one that invents a message
    /// is dead weight. This is what catches both.
    #[test]
    fn every_translation_has_the_same_messages_as_english() {
        let english = ids("en");
        assert!(english.len() > 50, "the English catalogue was not read");

        for locale in locales() {
            if locale == "en" {
                continue;
            }
            let theirs = ids(&locale);
            let missing: Vec<_> = english.difference(&theirs).collect();
            let extra: Vec<_> = theirs.difference(&english).collect();
            assert!(
                missing.is_empty() && extra.is_empty(),
                "{locale}: missing {missing:?}, unknown {extra:?}"
            );
        }
    }

    /// Every catalogue parses and loads.
    ///
    /// A Fluent slip - an unclosed placeable, a selector with no default -
    /// makes a catalogue load empty at runtime rather than fail the build, so
    /// each one is loaded here and asked for a message.
    #[test]
    fn every_translation_loads() {
        use i18n_embed::LanguageLoader;

        for locale in locales() {
            let language: i18n_embed::unic_langid::LanguageIdentifier = locale
                .parse()
                .unwrap_or_else(|error| panic!("{locale} is not a language tag: {error}"));
            let loader = i18n_embed::fluent::fluent_language_loader!();
            loader
                .load_languages(&super::Localizations, &[language])
                .unwrap_or_else(|error| panic!("{locale} did not load: {error}"));

            assert!(
                !loader.get("add-note").is_empty(),
                "{locale} loaded without its messages"
            );
            // The plural selectors are where a catalogue usually breaks.
            assert!(
                !loader
                    .get_args("in-minutes", maplit_count(5))
                    .contains("in-minutes"),
                "{locale}: in-minutes did not resolve"
            );
        }
    }

    /// One `count` argument, as the UI passes it.
    fn maplit_count(count: i64) -> std::collections::HashMap<&'static str, i64> {
        std::collections::HashMap::from([("count", count)])
    }

    /// The locale directories under `i18n/`.
    fn locales() -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("i18n");
        let mut locales: Vec<String> = std::fs::read_dir(dir)
            .expect("the translations are in the crate")
            .filter_map(|entry| {
                let entry = entry.ok()?;
                entry.file_type().ok()?.is_dir().then_some(())?;
                entry.file_name().into_string().ok()
            })
            .collect();
        locales.sort();
        locales
    }

    /// The message ids a catalogue defines: every line that starts one.
    fn ids(locale: &str) -> std::collections::BTreeSet<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("i18n")
            .join(locale)
            .join("fleck.ftl");
        let catalogue = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        catalogue
            .lines()
            .filter_map(|line| {
                // A continuation line is indented; a comment starts with #.
                let (id, _) = line.split_once(" = ")?;
                (!id.starts_with([' ', '#', '*', '[', '}'])).then(|| id.trim().to_string())
            })
            .collect()
    }
}
