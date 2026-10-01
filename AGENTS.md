# Fleck — project instructions

Sticky notes for the COSMIC desktop, in Rust with libcosmic. Private repo:
`github.com/joelebukatobi/fleck`. Branches: work on `dev`; `master` trails it.

## Layout

- `core/` — crate `fleck-core`: notes, storage, window state, image links. No GUI
  dependencies, so it is unit-testable on its own.
- `fleck/` — crate `fleck`: the app, the notes list, note windows, and the panel
  applet (`fleck --applet`; one binary does both).
- `data/` — desktop entries, AppStream metainfo, D-Bus service file, bundled
  Iconoir icons.
- `docs/ux.md` — the product's behaviour in plain language. **Update it with every
  behaviour or look change**; it is the source of truth for how Fleck should act.
- `docs/superpowers/` — planning notes, local only. Never commit (see `.gitignore`).

## Working rules

1. **Every commit must pass** these, in this order, before `git commit`:
   ```bash
   cargo fmt --all
   just check    # clippy -D warnings, the tests, and metadata validation
   ```
   CI runs the same. `just` recipes also cover building, installing and
   fetching the speech model; see the `justfile`.
2. **No AI attribution** in commits or PRs: no `Co-Authored-By`, no "Generated with"
   line. Commits are authored by Joel Onwuanaku <joelebuka@gmail.com>.
3. **Release builds for anything you look at**: debug libcosmic is very slow.
   `cargo build --release`, then `fleck --quit; target/release/fleck`.
4. **Don't launch the GUI on the user's desktop** unasked, and don't touch their real
   notes (`~/.local/share/fleck`) or panel configuration without asking.
5. **Push only when asked.**

## Conventions

- **Lints:** Clippy pedantic workspace-wide, with opt-outs listed in the root
  `Cargo.toml`. Deliberate exceptions are allowed in place, each with a reason.
- **Text:** every user-visible string goes through `fl!()` into
  `fleck/i18n/en/fleck.ftl`. Message ids are checked at compile time.
- **Icons:** Iconoir SVGs in `data/icons/iconoir/`, bundled with `include_bytes!` and
  drawn in the theme's colour. Stroke 2 for the panel icon, 1.5 for the rest.
- **Spacing:** multiples of 8 unless a platform number forces otherwise (libcosmic's
  header-bar padding is 7, for example).
- **Colours:** `fleck/src/palette.rs`. A note's colour is used as-is in light and dark
  mode; lines are a darker shade, headings a deeper one, text dark or white by
  contrast (WCAG AA, checked by tests).
- **Notes on disk:** `~/.local/share/fleck/notes/<uuid>.md`, TOML frontmatter plus
  Markdown body; images in `~/.local/share/fleck/notes/<uuid>/`.

## Platform notes worth keeping

- Note windows need `note_window_settings` (transparent, application id, no
  decorations) or they flicker.
- libcosmic only draws `header_start`/`dialog` on the **main** window, so note windows
  draw their own header bar and dialogs (a modal popover).
- COSMIC's clipboard carries text only: images go through `arboard`.
- File drops arrive through libcosmic's `dnd_destination` (`text/uri-list`); the winit
  file-drop event is X11-only.
- libcosmic blurs only the main window, so note windows enable blur themselves when
  COSMIC's frosted glass is on.
