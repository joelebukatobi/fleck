# Handoff

_Last updated: 2026-10-01_

## Task

Notes, images, reminders and dictation are all built. **Packaging is the only phase
left**, and it is deliberately last.

## State

- Branch `dev`, clean tree, **35 commits ahead of `origin/dev`** (images, reminders,
  dictation, metadata files). Push when Joel says so.
- `master` trails `dev` by everything since the icon work.
- Tests: 129 in `fleck`, 94 in `fleck-core`. Clippy clean at pedantic.

## Done recently

- **Reminders:** their own items (text, description, event + location, date, time,
  repeat), optionally about a note; `reminders.toml`. Notes/Reminders dropdown and a
  **+** in the list header. Firing gives a notification, plays Fleck's own sound for
  ten seconds, and opens the linked note. One-offs are kept as **Done**; **Clear
  finished** forgets them. `fleck --background` + autostart entry fire reminders while
  Fleck is closed.
- **Dictation:** a round microphone button floats over each note's bottom-right corner.
  Records with `pw-record`, transcribes with Whisper (whisper-rs, `base` model in
  `~/.local/share/fleck/models/`), types the result at the cursor. Needs `libclang-dev`
  and `cmake` to build.
- AppStream metainfo, D-Bus service file, autostart entry.
- **`justfile`**: build, check, validate, install/uninstall (system and user), model.
- **Fleck's own icon** (`data/icons/fleck/`): colour for the launcher, symbolic for
  the panel, replacing Iconoir's page-edit. Installed into
  `~/.local/share/icons/hicolor/scalable/apps/` for this user.

## Next

1. Push the waiting commits to `dev` (ask first).
2. Packaging: `.deb`, screenshots for the AppStream file, and how Fleck reaches other
   people (parked deliberately).
3. The panel icon only changes after `cargo build --release` **and** a
   `systemctl --user restart cosmic-panel`; the panel does not respawn an applet
   it has killed.

## Decisions

- Reminders live beside notes, not inside them; one file for all of them.
- Firing: notification **and** the note opens; alarm repeats for ten seconds.
- New reminders default to ten minutes ahead, rounded to five; the form says how far
  away the time is.
- Dialogs are modal and nothing dims behind them (COSMIC doesn't either).
- Pick-or-dismiss dialogs close from a corner button; confirmations keep Cancel.
- Existing notes stay yellow; new notes are Default.
- The icon is drawn by hand, not from an icon set; the design tool's provenance
  metadata is stripped (8 KB of an 8.5 KB file).

## Open, small

- The speech model is downloaded by hand (README); Fleck could fetch it on first use.
- Image lines read `![](<uuid>.png)`; shorter names were offered, never decided.
- `New note` (the empty-note label) lives in `fleck-core` and isn't translatable.
- Notification banners never pop on this desktop, for Fleck or anything else; only the
  tray shows them.

## Commands

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo build --release
fleck --quit; ~/Projects/sticky-notes/target/release/fleck      # look at it
fleck --quit; RUST_LOG=fleck=debug ~/Projects/sticky-notes/target/release/fleck
systemctl --user restart cosmic-panel    # after a rebuild: the panel does not
                                         # respawn a killed applet by itself
```
