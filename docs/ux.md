# Tack — how it should behave

Plain notes on the experience we want. Not a spec.

## Opening Tack

- When you open Tack, the **notes list** opens: every note you have, and a way
  to make a new one.
- If you had notes open when you last quit, a **dialog** appears over the list,
  dimming it, asking whether to reopen them.
  - **Reopen** — takes the theme accent colour. Those notes open, and the list closes.
  - **No thanks** — theme default. The dialog goes away and you stay in the list.
- If nothing was open when you last quit, you just get the list. No bar.
- "Open when you last quit" includes the last note you closed, since closing your
  last window is how you quit.
- Notes deleted since then are left out.

## The notes list

Modelled on the Windows Sticky Notes list window.

- **Header:** a `+` button on the left for a new note, then **Tack**, then the
  window buttons on the right.
- **Search bar** under the header. Filters as you type, matching note names and text.
- **Notes as cards**, most recently edited first, scrolling when there are more
  than fit. Each card shows the note's name in bold, the first couple of lines
  of its text, and when it was last edited.
- **Plain cards for now** — theme defaults. Card colours come with the design pass.
- Renaming a note is an action on its card.
- The darker panel the cards sit on has an **even gap on all four sides** between
  it and the window edge — including the bottom, which currently runs flush.

## Notes

- Tack is sticky notes, not a notes app. Each note is its own window; the list
  never turns into a note.

- A note opens in its own window.
- Closing a note does not delete it. It goes back to the list.
- Deleting a note is its own separate action.
- An empty note — no name, no text — is thrown away when you close it.
- A new note opens at **512 × 768** — half the width of a normal COSMIC window,
  same height. Where it appears on screen is up to COSMIC.
- If you resize a note, it reopens at that size next time.

## Window title bar

- Every note window's title reads **Note name | Tack**, followed by minimise,
  maximise and close. COSMIC already renders it this way.

## Inside a note

- Just the note text. No name field inside the note — the name lives in the title bar.
- A note's name is its first line of text. Renaming explicitly happens from the
  notes list.
- The note text has no border.
- The note text is ruled like lined paper: evenly spaced dotted lines, one per
  line of text, filling the window whether or not there is text on them.
- Pressing Enter moves the cursor down onto the next line.
- Resizing the window fills the new space with lines.
- Text must sit on the lines, and the lines scroll with the text.

## Look

- Bare minimum for now. Theme defaults for every colour, including the lines.
- Design comes later.
