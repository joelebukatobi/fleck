# Fleck

Sticky notes for the [COSMIC](https://system76.com/cosmic) desktop.

Each note is its own window. A notes list, modelled on Windows Sticky Notes,
lets you search, open, rename and delete notes, and a panel icon brings the
list up from anywhere. Fleck is written in Rust with
[libcosmic](https://github.com/pop-os/libcosmic), so it follows your COSMIC
theme, accent colour and light or dark mode.

Fleck is early software. There are no packages yet; build it from source.

## Features

- A note per window, on lined paper, saved as you type.
- A notes list with search, most recently edited first.
- Rename and delete from the list, with a confirmation before deleting.
- Offers to reopen the notes you had open when you last quit.
- A COSMIC panel applet that opens the list, starting Fleck if it isn't running.
- Undo and redo in notes (Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y).
- Notes are plain Markdown files on disk.

## Building

You need a Rust toolchain ([rustup](https://rustup.rs)) and these development
packages (Pop!_OS and Ubuntu names):

```bash
sudo apt install pkg-config libxkbcommon-dev libwayland-dev libfontconfig-dev libexpat1-dev libfreetype6-dev
```

Then:

```bash
cargo build --release
```

The program is `target/release/fleck`. Use release builds for anything but
development: debug builds of libcosmic are very slow.

## Installing

Copy the program somewhere on your `PATH`, install the two desktop entries (one
for the app, one for the panel applet) and the panel applet's icon:

```bash
install -Dm755 target/release/fleck ~/.local/bin/fleck
install -Dm644 data/io.github.joelebukatobi.Fleck.desktop ~/.local/share/applications/
install -Dm644 data/io.github.joelebukatobi.FleckApplet.desktop ~/.local/share/applications/
install -Dm644 data/icons/iconoir/page-edit.svg ~/.local/share/icons/hicolor/scalable/apps/io.github.joelebukatobi.FleckApplet-symbolic.svg
install -Dm644 data/io.github.joelebukatobi.Fleck.metainfo.xml ~/.local/share/metainfo/
sed "s|^Exec=.*|Exec=$HOME/.local/bin/fleck|" data/io.github.joelebukatobi.Fleck.service \
  > ~/.local/share/dbus-1/services/io.github.joelebukatobi.Fleck.service
```

The last line lets the session start Fleck on demand: with it in place, `fleck --new-note`
and the panel icon work even when Fleck isn't running. The AppStream file is what software
centres read to list Fleck.

To add the panel icon, open COSMIC Settings, go to Desktop, then Panel, then
Configure panel applets, and add **Fleck**.

## Command line

The app and the panel applet are the same program:

| Command | What it does |
| --- | --- |
| `fleck` | Opens the notes list, or brings it forward if Fleck is already running. |
| `fleck --applet` | Runs the panel applet. The panel starts this for you. |
| `fleck --list` | Prints every note: id, name, and whether it is open. |
| `fleck --new-note` | Creates a note and prints its id. |
| `fleck --show <id>` | Opens a note. |
| `fleck --hide <id>` | Closes a note's window without deleting it. |
| `fleck --delete <id>` | Deletes a note. |
| `fleck --toggle-all` | Hides every open note, or shows them all if none are open. |
| `fleck --quit` | Quits the running Fleck. |

Only one Fleck runs at a time. The commands above talk to it over D-Bus as
`io.github.joelebukatobi.Fleck`.

## Where your notes live

- Notes: `~/.local/share/fleck/notes/`, one Markdown file per note.
- Window sizes and the notes open at last quit: `~/.local/state/fleck/windows.toml`.

Both follow `XDG_DATA_HOME` and `XDG_STATE_HOME` if you set them.

## Logs

Fleck logs warnings and errors to stderr. For more detail:

```bash
RUST_LOG=fleck=debug fleck
```

## Translating

All text shown in Fleck lives in `fleck/i18n/en/fleck.ftl`, in
[Fluent](https://projectfluent.org) format. To add a language, copy that file
to `fleck/i18n/<language code>/fleck.ftl` and translate the values.

## Contributing

Before sending a change, make sure these pass. CI runs the same checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The code is a Cargo workspace:

- `core/`: notes, storage and window state, with no GUI dependencies.
- `fleck/`: the app, the notes list, note windows and the panel applet.

## License

Fleck is licensed under the GNU General Public License, version 3 or later.
See [LICENSE](LICENSE).

Icons are from [Iconoir](https://iconoir.com), used under the MIT
License. See [NOTICE](NOTICE).
