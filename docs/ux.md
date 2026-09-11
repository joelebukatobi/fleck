# Fleck — how it should behave

Plain notes on the experience we want. Not a spec.

## Opening Fleck

- When you open Fleck, the **notes list** opens: every note you have, and a way
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

- **Header:** a `+` button on the left for a new note, then **Fleck**, then the
  window buttons on the right.
- **Search bar** under the header. Filters as you type, matching note names and text.
- **Notes as cards**, most recently edited first, scrolling when there are more
  than fit. Each card shows the note's name in bold, the first couple of lines
  of its text, and when it was last edited.
- **Plain cards for now** — theme defaults. Card colours come with the design pass.
- Renaming a note is an action on its card.
- Deleting a note is an action on its card: a trash icon after the rename pencil.
  It asks first — "Delete note?" with **Delete** (destructive style) and
  **Cancel**. If the note is open in a window, that window closes.
- The darker panel the cards sit on has an **even gap on all four sides** between
  it and the window edge — including the bottom, which currently runs flush.

## Notes

- Fleck is sticky notes, not a notes app. Each note is its own window; the list
  never turns into a note.

- A note opens in its own window.
- Closing a note does not delete it. It goes back to the list.
- Deleting a note is its own separate action.
- An empty note — no name, no text — is thrown away when you close it.
- A new note opens at **512 × 768** — half the width of a normal COSMIC window,
  same height. Where it appears on screen is up to COSMIC.
- If you resize a note, it reopens at that size next time.

## Window title bar

- Every note window's title reads **Note name | Fleck**, followed by minimise,
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

- Mostly theme defaults for now; the full design pass comes later.
- Working in multiples of 8.
- Notes list sizes, fixed rather than following COSMIC's roundness setting:
  - Content panel: 8 px padding on all four sides.
  - Search bar: 8 px padding left and right, 4 px corner radius.
  - Cards: 4 px corner radius, 8 px between cards.
  - Card heading: a darker strip across the top of the card, edge to edge, with
    the title on the left and the rename pencil on the right of the same row.
    8 px padding inside it. Darker is derived from the card's theme colour.
  - 8 px between the heading strip and the card's content.
  - Card content (preview and time): 8 px padding, including top and bottom.
  - Rename and delete icons sit 8 px apart; the delete icon is 8 px from the
    card's right edge, in line with the search bar's clear icon.
  - The search magnifier lines up with the card titles on the left.
  - Icon buttons (+, rename, delete, search clear) show no background on hover;
    the icon turns the accent colour, or red for delete.
  - Hovering a card lightens it slightly; pressing darkens it slightly. The card
    itself is the clickable surface, so the effect lines up with its edges.
  - The card list's scrollbar is slim until the mouse is over it, then widens,
    like other COSMIC apps.
  - The scrollbar thumb runs the full height of the list, with no inset at the
    top or bottom.
  - At least 4 px between the cards and the scrollbar at its widest. With no
    scrollbar, cards run full width, lined up with the search bar.

## Panel applet

- An icon that lives permanently in the COSMIC top panel: `fleck --applet`, the
  same program as the app, not a separate binary.
- Clicking it opens the full notes list window — the same as launching Fleck from
  the app library. No popup of its own.
- If Fleck isn't running, clicking it starts Fleck.

## Dialogs

- Dialogs over the notes list are 75% of the list window's width.
- The list behind a dialog is not dimmed or blurred.

## Icons

- Fleck uses its own bundled icons from **Phosphor** (MIT), not the system icon theme,
  so they look the same whatever COSMIC's icon theme is set to.
- **Bold** weight — the closest to COSMIC's own icons. COSMIC's measure 2 px thick at
  16 px; Phosphor bold is 1.5 px, regular would be 1 px.
- Icons follow the theme's text colour, so they work in light and dark mode.
- Fleck's own app icon, for the panel list and the app library, comes with the design pass.

