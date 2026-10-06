# Handoff

_Last updated: 2026-10-05_

## Task

Notes, images, reminders and dictation are all built. What is left is **getting Fleck
to people**: the Flatpak is built and installs, but it is not submitted, and there
are no screenshots.

## State

- Branch `dev`, **9 commits ahead of `origin/dev`** (everything from the audio work
  onwards). Push when Joel says so; the repo is public now.
- `master` trails `dev` by everything since the icon work.
- Tests: 135 in `fleck` (2 ignored, needing a microphone or the network), 97 in
  `fleck-core`. Clippy clean at pedantic.

## Done recently

- **Reminders:** their own items (text, description, event + location, date, time,
  repeat), optionally about a note; `reminders.toml`. Notes/Reminders dropdown and a
  **+** in the list header. Firing gives a notification, plays Fleck's own sound for
  ten seconds, and opens the linked note. One-offs are kept as **Done**; **Clear
  finished** forgets them. `fleck --background` + autostart entry fire reminders while
  Fleck is closed.
- **Dictation:** a round microphone button floats over each note's bottom-right corner.
  Records through `audio.rs` (PulseAudio's simple API, in-process), transcribes with
  Whisper (whisper-rs, `base` model in `~/.local/share/fleck/models/`), types the
  result at the cursor. The **first press fetches the model** (142 MB, `ureq`, written
  to a `.part` file and renamed). Needs `libpulse-dev`, `libclang-dev` and `cmake` to
  build.
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
- **Flatpak** (2026-10-05): `flatpak/io.github.joelebukatobi.Fleck.json` +
  `cargo-sources.json`, `just flatpak` builds and installs it, and it runs. Audio and
  the model download moved in-process so neither needs a binary the sandbox lacks;
  libcosmic unpinned so the crates can be vendored (cost one `Scrollable` API port);
  the applet's desktop entry and icon are installed under dotted names, because
  flatpak only exports files named after the app id; the D-Bus service file's `Exec`
  is rewritten to `/app/bin/fleck`.
- **First-run import**: a sandboxed Fleck copies in a system install's notes, images
  and reminders and leaves the originals alone. Verified with the real store.
- **Eight languages** and two tests that keep the catalogues in step.
- **BUILD.md, CONTRIBUTING.md, CHANGELOG.md**, and a README that points at them.

## Next

1. **Screenshots** - Joel's to take, nobody else can. They go in the AppStream
   metainfo (which has none, so a store listing looks empty) and at the top of the
   README, the way Clipboard Manager's does.
2. **Submit the Flatpak.** Flathub first, per `pop-os/cosmic-flatpak`'s own README
   ("For COSMIC apps, please first try to submit to Flathub"); that remote is the
   fallback for what Flathub won't take. A submission is one directory:
   `app/<app-id>/<app-id>.json` plus `cargo-sources.json`.
3. **Joel's three manual steps for the apt side**, if it ships alongside:
   1. `gpg --quick-generate-key "Fleck Packages <joelebuka@gmail.com>" rsa4096 sign never`
   2. `gpg --export "Fleck Packages" > packaging/apt/fleck.gpg` and commit it.
   3. Add the private half as the `APT_SIGNING_KEY` secret, and turn on Pages
      (Settings, Pages, Source, branch `gh-pages`, root) once the branch exists.
4. Then `git tag -a v0.1.0 -m 'Fleck 0.1.0' && git push origin v0.1.0` releases it.
5. Push the waiting commits to `dev` (ask first), and decide whether `master`
   catches up.
6. Delete the leftover `fleck-apt` repo - needs `gh auth refresh -s delete_repo`, or
   do it in its Settings.

## Decisions

- Reminders live beside notes, not inside them; one file for all of them.
- Firing: notification **and** the note opens; alarm repeats for ten seconds.
- New reminders default to ten minutes ahead, rounded to five; the form says how far
  away the time is.
- Dialogs are modal and nothing dims behind them (COSMIC doesn't either).
- Pick-or-dismiss dialogs close from a corner button; confirmations keep Cancel.
- Existing notes stay yellow; new notes are Default.
- **Distribution:** Flatpak is the default, because that is how every other COSMIC
  applet reaches people and what the COSMIC Store serves - Status Hub and Clipboard
  Manager both ship that way and nothing else. The `.deb` and the signed apt
  repository on `gh-pages` stay as the alternative for people who prefer apt. No PPA
  (Launchpad's builders have no network, so every crate would need vendoring), no
  `curl | sh`.
- **Corrected 2026-10-05:** an earlier decision here claimed a sandboxed app cannot
  be a COSMIC applet, and ruled Flatpak out on that basis. It was wrong - the panel
  launches applets from `flatpak run` through their exported desktop entries, and
  `com.system76.Cosmic.BaseApp` exists for exactly this. The reasoning, not just the
  conclusion, is what needed fixing.
- Two things follow from shipping in a sandbox: **no shelling out** to binaries that
  are not in the runtime (hence `audio.rs` and `ureq`), and **libcosmic stays
  unpinned**, because a pinned rev puts two commits of the same repository in the
  lock and Cargo cannot vendor that.
- **The repo is public** as of 2026-10-05, so Pages can serve the apt index from it;
  the separate `fleck-apt` repo was dropped as unnecessary.
- The icon is drawn by hand, not from an icon set; the design tool's provenance
  metadata is stripped (8 KB of an 8.5 KB file).

## Open, small

- The Flatpak is built and installed locally but **not submitted** to
  `pop-os/cosmic-flatpak` or Flathub, which is the one thing left between Fleck and
  the users the other COSMIC applets have.
- **No screenshots**: the AppStream metainfo has none, and neither does the README.
  Both the store listing and the front page want them, and only Joel can take them.
- The seven new translations are unreviewed by native speakers.
- In the sandbox, reminders cannot fire while Fleck is closed: the autostart entry
  can't be installed from a Flatpak, so that needs the background portal.
- Image lines read `![](<uuid>.png)`; shorter names were offered, never decided.
- `New note` (the empty-note label) lives in `fleck-core` and isn't translatable.
- Notification banners never pop on this desktop, for Fleck or anything else; only the
  tray shows them.

## Commands

```bash
just check                               # everything CI runs
cargo test -- --ignored                  # the microphone and download checks
cargo build --release
fleck --quit; ~/Projects/sticky-notes/target/release/fleck      # look at it
fleck --quit; RUST_LOG=fleck=debug ~/Projects/sticky-notes/target/release/fleck
just deb                                 # the package, target/deb/
just apt-repo                            # the signed apt repository, target/apt/
just flatpak                             # build and install the Flatpak
flatpak run io.github.joelebukatobi.Fleck
flatpak kill io.github.joelebukatobi.Fleck
systemctl --user restart cosmic-panel    # after a rebuild: the panel does not
                                         # respawn a killed applet by itself
```
