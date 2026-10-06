# Fleck releases

## Unreleased

Everything so far: Fleck has not had a release yet.

- **Notes**: a window each, on lined paper, saved as you type, as plain Markdown
  files on disk. A list with search, most recent first. Rename, colour and
  delete from a note's own menu or its card. Undo and redo. Fleck offers to
  reopen the notes you had open when you last quit.
- **Note colours**: the classic yellow, a colour that follows your theme, and
  one for each of eight Linux distributions, with lines, heading strips and text
  derived from the colour so it reads the same in light and dark.
- **Images**: paste or drop a picture into a note. Thumbnails sit under the
  text; clicking one opens it full size, to copy or delete.
- **Reminders**: their own items, optionally about a note — text, description,
  a location when it is an event, a date and time, and simple repeats. A
  reminder coming due raises a notification, sounds an alarm for ten seconds and
  opens the note it is about. Finished one-off reminders are kept as **Done**.
  `fleck --background` fires reminders while Fleck is closed.
- **Dictation**: a microphone in the corner of every note transcribes speech
  with Whisper, entirely on this machine, and types it at the cursor. A live
  preview shows what has been said so far. The speech model, about 142 MB, is
  fetched on the first press.
- **The panel applet**: `fleck --applet`, the same binary, opens the list and
  starts Fleck if it is not running.
- **Fleck's own icon**, a note with a folded corner and a single cyan fleck.
- **Eight languages**: English, German, Spanish, French, Italian, Dutch,
  Brazilian Portuguese and Simplified Chinese. All but English are unreviewed by
  native speakers; corrections are welcome.
- **Packages**: a `.deb`, a signed apt repository served from this repository's
  `gh-pages` branch, and a Flatpak for the COSMIC Store. A sandboxed Fleck
  copies in the notes of a system install the first time it runs.
