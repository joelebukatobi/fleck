# Fleck

Sticky notes for the [COSMIC](https://system76.com/cosmic) desktop.

Each note is its own window. A notes list, modelled on Windows Sticky Notes,
lets you search, open, rename and delete notes, and a panel icon brings the
list up from anywhere. Fleck is written in Rust with
[libcosmic](https://github.com/pop-os/libcosmic), so it follows your COSMIC
theme, accent colour and light or dark mode.

Fleck is early software. Install it from Fleck's apt repository (below), or
build it from source.

## Installing on Pop!_OS, Ubuntu or Debian

Add Fleck's apt repository once, and new versions arrive with the rest of your
system updates:

```bash
curl -fsSL https://joelebukatobi.github.io/fleck/fleck.gpg | sudo tee /usr/share/keyrings/fleck.gpg > /dev/null
echo 'deb [signed-by=/usr/share/keyrings/fleck.gpg] https://joelebukatobi.github.io/fleck stable main' | sudo tee /etc/apt/sources.list.d/fleck.list
sudo apt update && sudo apt install fleck
```

Or install a single `.deb` from the [releases
page](https://github.com/joelebukatobi/fleck/releases) without adding the
repository, if you'd rather update by hand:

```bash
sudo apt install ./fleck_0.1.0_amd64.deb
```

Either way, that puts Fleck in the app library, adds the panel applet (COSMIC Settings,
Desktop, Panel, Configure panel applets) and starts the reminder watcher with
your session. `sudo apt remove fleck` takes it all away again.

Dictation needs a speech model, about 142 MB. The first press of a note's
microphone fetches it; `just model` gets it ahead of time instead.

On other distributions, build from source.

## Features

- A note per window, on lined paper, saved as you type.
- A notes list with search, most recently edited first.
- Rename, recolour and delete a note from its own menu or its card.
- Nine note colours: the classic yellow, one that follows your theme, and one
  for each of eight Linux distributions.
- Paste or drop images into a note; thumbnails sit under the text.
- Reminders, optionally about a note, with repeats, a notification and an alarm.
- Dictation: speak into a note and Whisper types it, entirely on this machine.
- Offers to reopen the notes you had open when you last quit.
- A COSMIC panel applet that opens the list, starting Fleck if it isn't running.
- Undo and redo in notes (Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y).
- Notes are plain Markdown files on disk.
- Speaks eight languages.

## Building

[BUILD.md](BUILD.md) has the dependencies, the build, the test commands and how
the packages are made. The short version, with a Rust toolchain and
[just](https://github.com/casey/just) installed:

```bash
sudo apt install pkg-config libxkbcommon-dev libwayland-dev libfontconfig-dev libexpat1-dev libfreetype6-dev libpulse-dev cmake libclang-dev
just build-release
```

## Installing

With [just](https://github.com/casey/just):

```bash
just install-user    # this user only: no root needed
just model           # the speech model for dictation, about 142 MB
```

`just install` installs for everyone (needs root), and `just uninstall-user` or
`just uninstall` removes it again. `just check` runs everything CI runs, and
`just deb` builds the package described above.

By hand, if you'd rather: copy the program somewhere on your `PATH`, install the two desktop entries (one
for the app, one for the panel applet) and Fleck's icons:

```bash
install -Dm755 target/release/fleck ~/.local/bin/fleck
install -Dm644 data/io.github.joelebukatobi.Fleck.desktop ~/.local/share/applications/
install -Dm644 data/io.github.joelebukatobi.FleckApplet.desktop ~/.local/share/applications/
install -Dm644 data/icons/fleck/io.github.joelebukatobi.Fleck-symbolic.svg ~/.local/share/icons/hicolor/scalable/apps/io.github.joelebukatobi.FleckApplet-symbolic.svg
install -Dm644 data/icons/fleck/io.github.joelebukatobi.Fleck.svg ~/.local/share/icons/hicolor/scalable/apps/io.github.joelebukatobi.Fleck.svg
install -Dm644 data/io.github.joelebukatobi.Fleck.metainfo.xml ~/.local/share/metainfo/
sed "s|^Exec=.*|Exec=$HOME/.local/bin/fleck|" data/io.github.joelebukatobi.Fleck.service \
  > ~/.local/share/dbus-1/services/io.github.joelebukatobi.Fleck.service
```

For reminders to fire while Fleck is closed, also install the autostart entry:

```bash
install -Dm644 data/io.github.joelebukatobi.Fleck-autostart.desktop ~/.config/autostart/
```

The D-Bus service line lets the session start Fleck on demand: with it in place, `fleck --new-note`
and the panel icon work even when Fleck isn't running. The AppStream file is what software
centres read to list Fleck.

To add the panel icon, open COSMIC Settings, go to Desktop, then Panel, then
Configure panel applets, and add **Fleck**.

## Dictation

The microphone button in a note turns speech into text with
[Whisper](https://github.com/ggerganov/whisper.cpp), on this machine. Fleck needs the
speech model, about 142 MB, once:

```bash
mkdir -p ~/.local/share/fleck/models
curl -L https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin \
  -o ~/.local/share/fleck/models/ggml-base.bin
```

Recording uses `pw-record` or `parecord`, which come with PipeWire and PulseAudio.
Building Fleck with dictation needs `libclang-dev` and `cmake` for whisper.cpp.

## Command line

The app and the panel applet are the same program:

| Command | What it does |
| --- | --- |
| `fleck` | Opens the notes list, or brings it forward if Fleck is already running. |
| `fleck --applet` | Runs the panel applet. The panel starts this for you. |
| `fleck --background` | Runs with no windows, waiting to fire reminders. Used by the autostart entry. |
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

- Reminders: `~/.local/share/fleck/reminders.toml`.

Both follow `XDG_DATA_HOME` and `XDG_STATE_HOME` if you set them.

Installed as a Flatpak, Fleck is sandboxed and keeps its own copy under
`~/.var/app/io.github.joelebukatobi.Fleck/`. The first time it runs it copies in
the notes of a system install, if there is one, and leaves that install's own
notes alone.

## Logs

Fleck logs warnings and errors to stderr. For more detail:

```bash
RUST_LOG=fleck=debug fleck
```

## Translating

Fleck speaks English, German, Spanish, French, Italian, Dutch, Brazilian
Portuguese and Simplified Chinese. All but English are unreviewed by native
speakers, so corrections are welcome; [CONTRIBUTING.md](CONTRIBUTING.md) says
how to send one or add a language.

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) has the house rules, [BUILD.md](BUILD.md) the
build, and [docs/ux.md](docs/ux.md) describes how Fleck is meant to behave -
it is the source of truth for the product. [CHANGELOG.md](CHANGELOG.md) says
what changed in each release.

The code is a Cargo workspace:

- `core/`: notes, storage, reminders and window state, with no GUI dependencies.
- `fleck/`: the app, the notes list, note windows and the panel applet.

## License

Fleck is licensed under the GNU General Public License, version 3 or later.
See [LICENSE](LICENSE).

Icons are from [Iconoir](https://iconoir.com), used under the MIT
License. See [NOTICE](NOTICE).
