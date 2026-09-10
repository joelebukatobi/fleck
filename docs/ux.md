# Tack — how it should behave

Plain notes on the experience we want. Not a spec.

## Opening Tack

- When you open Tack, the **notes list** opens: every note you have, and a way
  to make a new one.
- If you had notes open when you last quit, the list shows a bar across the top
  asking whether to reopen them — like a browser offering to restore your tabs.
  - **Reopen** — those notes open, and the list closes.
  - **No** — the bar goes away and you stay in the list.
- If nothing was open when you last quit, you just get the list. No bar.
- "Open when you last quit" includes the last note you closed, since closing your
  last window is how you quit.
- Notes deleted since then are left out.

## Notes

- A note opens in its own window.
- Closing a note does not delete it. It goes back to the list.
- Deleting a note is its own separate action.
- An empty note — no name, no text — is thrown away when you close it.
- A new note opens at **512 × 768** — half the width of a normal COSMIC window,
  same height. Where it appears on screen is up to COSMIC.
- If you resize a note, it reopens at that size next time.

## Window title bar

- The left side of every Tack window says **Tack**, so you can tell what app it is.
- The note's name sits in the middle.
- Minimise, maximise and close stay on the right.

## Inside a note

- Name at the top, note text below. Neither has a border.
- A thin horizontal rule separates them, with a little space above and below it.
- The note text is ruled like lined paper: evenly spaced dotted lines, one per
  line of text, filling the window whether or not there is text on them.
- Pressing Enter moves the cursor down onto the next line.
- Resizing the window fills the new space with lines.
- Text must sit on the lines, and the lines scroll with the text.

## Look

- Bare minimum for now. Theme defaults for every colour, including the lines.
- Design comes later.
