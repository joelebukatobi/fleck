# Handoff

_Last updated: 2026-09-30_

## Task

Fleck is feature-complete for notes, colours and images. Next up, in this order:
**reminders (Phase 2)**, then **voice input (Phase 3)**, then **packaging last**.

## State

- Branch `dev`, clean tree, **10 commits ahead of `origin/dev`** (images, dialog
  fixes, AppStream and D-Bus files). Push when Joel says so.
- `master` trails `dev` by the whole images round; no merge since the icon work.
- Tests: 118 in `fleck`, 80 in `fleck-core`. Clippy clean at pedantic.

## Done recently

- Images in notes: paste (Ctrl+V), drag-and-drop, thumbnail row, click a thumbnail to
  put the cursor on its line and open the picture, Copy and Delete, files under
  `notes/<uuid>/` with plain Markdown links in the body.
- Image dialog: no title (the file name is a uuid), picture scaled to fit 320 px, close
  button in the top-right corner. The colour dialog closes the same way.
- Escape closes a note's dialog or its Settings menu.
- `data/io.github.joelebukatobi.Fleck.metainfo.xml` (AppStream) and
  `data/io.github.joelebukatobi.Fleck.service` (D-Bus activation), both installed by
  the README steps, metainfo validated in CI.

## Next

1. Push the waiting commits to `dev` (ask first).
2. Reminders: decide one-per-note vs several, repeats, what firing does (COSMIC
   notification, opening the note, or both), and whether they fire with Fleck closed —
   that last one is what the new D-Bus service file makes possible.
3. Voice input (Whisper, model as a dependency).
4. Packaging: `.deb`, Fleck's own app icon, `justfile`. How Fleck reaches other people
   is deliberately parked until then.

## Decisions

- Existing notes stay yellow; new notes are Default (the theme's own look).
- Distribution question (Flatpak vs `.deb` vs other) is deliberately last.
- Dialogs are modal: clicking outside does not dismiss them; Escape or a button does.
- Rename and Delete keep Cancel buttons; pick-or-dismiss dialogs use the corner close.

## Open, small

- `New note` (the label for an empty note) lives in `fleck-core` and can't be
  translated yet.
- Image lines read `![](<uuid>.png)`; shorter names were offered and not decided.
- Only English exists, though everything is routed through Fluent.

## Commands

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo build --release
fleck --quit; ~/Projects/sticky-notes/target/release/fleck   # look at it
pkill -f 'fleck --applet'                                    # panel restarts it
```
