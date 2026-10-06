# Fleck — project instructions

Sticky notes for the COSMIC desktop, in Rust with libcosmic. Public repo:
`github.com/joelebukatobi/fleck`. Branches: work on `dev`; `master` trails it;
`gh-pages` is the apt repository and is written only by the release workflow.

## Layout

- `core/` — crate `fleck-core`: notes, storage, window state, image links. No GUI
  dependencies, so it is unit-testable on its own.
- `fleck/` — crate `fleck`: the app, the notes list, note windows, and the panel
  applet (`fleck --applet`; one binary does both).
- `data/` — desktop entries, AppStream metainfo, D-Bus service file, Fleck's own
  icon, bundled Iconoir icons.
- `packaging/apt/` — `publish.sh` (builds and signs the apt index), `changelog.sh`,
  and the landing page the apt repository serves.
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
6. **Releases:** tagging `v*` runs `.github/workflows/release.yml`: it builds the
   `.deb`, attaches it to the GitHub release, and publishes it to the signed apt
   repository on this repo's `gh-pages` branch, served at
   `joelebukatobi.github.io/fleck`, so users get it with `apt upgrade`. That branch
   holds nothing but the packages, the index, the key and the changelog.
7. **Flatpak is the way COSMIC apps reach people**, through `pop-os/cosmic-flatpak`
   or Flathub, which is what the COSMIC Store serves; the `.deb` is the alternative,
   not the plan. `flatpak/io.github.joelebukatobi.Fleck.json` builds and installs
   (`just flatpak`) but is not submitted yet. Two consequences for the code: no
   shelling out to binaries that only exist outside a sandbox - audio goes through
   `audio.rs`, downloads through `ureq` - and **libcosmic must stay unpinned**, since
   a pinned rev puts two commits of the same repository in the lock and Cargo cannot
   vendor that. A PPA was ruled out: Launchpad's builders have no network, so every
   crate would need vendoring for the same result.

## Conventions

- **Lints:** Clippy pedantic workspace-wide, with opt-outs listed in the root
  `Cargo.toml`. Deliberate exceptions are allowed in place, each with a reason.
- **Text:** every user-visible string goes through `fl!()` into
  `fleck/i18n/en/fleck.ftl`. Message ids are checked at compile time.
- **Icons:** Fleck's own icon in `data/icons/fleck/` (colour for the launcher,
  symbolic for the panel); Iconoir SVGs at stroke 1.5 in `data/icons/iconoir/` for
  everything inside the app. All bundled with `include_bytes!` and tinted by the theme.
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
