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

## Window title bar

- The left side of every Tack window says **Tack**, so you can tell what app it is.
- The note's name sits in the middle.
- Minimise, maximise and close stay on the right.
