# Handoff

_Last updated: 2026-10-01_

## Task

Notes, images, reminders and dictation are all built. **Packaging is the only phase
left**, and it is deliberately last.

## State

- Branch `dev`, clean tree, **37 commits ahead of `origin/dev`** (images, reminders,
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
- **Packaging:** `just deb` builds a stripped 8 MB package with dpkg-deb (no
  packaging toolchain needed); `just apt-repo` builds the signed apt repository
  locally; `.github/workflows/release.yml` does both on a `v*` tag and pushes to
  this repo's `gh-pages` branch, served at `joelebukatobi.github.io/fleck`.
  Verified: real `apt update` + `apt-cache policy` against the generated repo.
- **Fleck's own icon** (`data/icons/fleck/`): colour for the launcher, symbolic for
  the panel, replacing Iconoir's page-edit. Installed into
  `~/.local/share/icons/hicolor/scalable/apps/` for this user.

## Next

1. **Joel's four manual steps before the first release can go out** (nothing else
   blocks it):
   1. Generate the signing key:
      `gpg --quick-generate-key "Fleck Packages <joelebuka@gmail.com>" rsa4096 sign never`
   2. Export the public half to `packaging/apt/fleck.gpg`
      (`gpg --export "Fleck Packages" > packaging/apt/fleck.gpg`) and commit it.
   3. Add the private half as the `APT_SIGNING_KEY` secret on `fleck`. No second
      token: the workflow pushes `gh-pages` with its own `GITHUB_TOKEN`.
   4. Turn on Pages: Settings, Pages, Source, branch `gh-pages`, root. The branch
      appears with the first release, so this step can wait until after the tag.
   5. Delete the leftover `fleck-apt` repo (needs `gh auth refresh -s delete_repo`,
      or do it in its Settings).
2. Push the waiting commits to `dev` (ask first).
3. Then `git tag -a v0.1.0 -m 'Fleck 0.1.0' && git push origin v0.1.0` releases it.
4. Still open: screenshots for the AppStream listing, and the COSMIC Store
   submission.
3. The panel icon only changes after `cargo build --release` **and** a
   `just deb                                 # the package, target/deb/
just apt-repo                            # the signed apt repository, target/apt/
systemctl --user restart cosmic-panel`; the panel does not respawn an applet
   it has killed.

## Decisions

- Reminders live beside notes, not inside them; one file for all of them.
- Firing: notification **and** the note opens; alarm repeats for ten seconds.
- New reminders default to ten minutes ahead, rounded to five; the form says how far
  away the time is.
- Dialogs are modal and nothing dims behind them (COSMIC doesn't either).
- Pick-or-dismiss dialogs close from a corner button; confirmations keep Cancel.
- Existing notes stay yellow; new notes are Default.
- **Distribution:** `.deb` plus a signed apt repository on this repo's `gh-pages`
  branch, so updates arrive with `apt upgrade`. No Flathub (a sandboxed app can't be
  a COSMIC applet), no PPA (Launchpad's builders have no network, so every crate
  would need vendoring), no `curl | sh`.
- **The repo is public** as of 2026-10-05, so Pages can serve the apt index from it;
  the separate `fleck-apt` repo was dropped as unnecessary.
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
just deb                                 # the package, target/deb/
just apt-repo                            # the signed apt repository, target/apt/
systemctl --user restart cosmic-panel    # after a rebuild: the panel does not
                                         # respawn a killed applet by itself
```
