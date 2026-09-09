mod app;
mod palette;

use sticky_notes_core::Store;

fn notes_dir() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("HOME is set");
            std::path::PathBuf::from(home).join(".local/share")
        });
    base.join("tack/notes")
}

fn main() -> cosmic::iced::Result {
    let store = Store::new(notes_dir());
    cosmic::app::run::<app::Tack>(
        cosmic::app::Settings::default().no_main_window(true),
        store,
    )
}
