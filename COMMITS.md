# Decision log

One line per decision or commit worth remembering. Newest last.

- 2026-09-04 — Sticky notes, not a notes app: each note is its own window; a notes
  list modelled on Windows Sticky Notes sits alongside.
- 2026-09-06 — Note windows flickered until they used the same window settings as
  libcosmic's main window (transparent, application id).
- 2026-09-07 — Layer-shell abandoned: the bottom layer draws over windows, the
  background layer gets no keyboard input. Notes are ordinary windows.
- 2026-09-09 — Renamed Tack to Fleck, with a one-time move of the data directory.
- 2026-09-11 — The panel applet merged into the `fleck` binary (`fleck --applet`),
  halving what gets installed.
- 2026-09-11 — Crates renamed to `fleck`/`fleck-core`; README, LICENSE, rustfmt,
  Clippy pedantic and CI added; planning notes removed from git history.
- 2026-09-11 — Review fixes: exits now save pending edits; a reopened list keeps its
  header and dialogs; note files must be named after their note.
- 2026-09-11 — Pushed to github.com/joelebukatobi/fleck (private), `master` default.
- 2026-09-17 — Icons switched to Iconoir; note windows draw their own title bar with a
  Settings menu (Rename, Colour, Delete, Notes, Theme).
- 2026-09-17 — Note colours: classic yellow plus Linux distribution colours, used
  as-is in both modes, with a darker shade for lines and a deeper one for card
  headings. Default is the theme's own look and the default for new notes.
- 2026-09-18 — Images in notes: paste and drag-and-drop, stored beside the note as
  plain Markdown links, with a thumbnail row and an image dialog.
- 2026-09-30 — AppStream metainfo and D-Bus service files added; CI validates the
  metainfo. Distribution (how Fleck reaches other people) parked until last.
- 2026-09-30 — Handoff files added (AGENTS, CLAUDE, HANDOFF, COMMITS).
- 2026-10-01 — Reminders built: own items with repeats, events and locations, notified
  with Fleck's own ten-second alarm, firing in the background through an autostart
  entry; fired one-offs are kept as Done.
- 2026-10-01 — Dictation built: pw-record plus Whisper through whisper-rs, typed in at
  the cursor. A killed recorder leaves a WAV claiming zero samples, so Fleck interrupts
  it and parses the file itself rather than trusting the header.
- Fleck's own icon replaces the page-edit stand-in: colour for the launcher, symbolic for the panel.
- Distribution decided: .deb on GitHub Releases plus a signed apt repository on the public fleck-apt's Pages; no Flathub (applets can't be sandboxed), no PPA (Launchpad builders are offline).
