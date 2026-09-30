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

- **Header:** an **Add Note** button on the left (the same text-button style as a
  note's Settings), **Fleck** in the middle, then the window buttons on the right.
- **Search bar** under the header. Filters as you type, matching note names and text.
  The placeholder starts right at the bar's padding, in line with the card titles.
  The magnifier sits at the right edge; there is no clear button.
- **Notes as cards**, most recently edited first, scrolling when there are more
  than fit. Each card shows the note's name in bold, the first couple of lines
  of its text, and when it was last edited.
- **Plain cards for now** — theme defaults. Card colours come with the design pass.
- Renaming a note is an action on its card.
- Deleting a note is an action on its card: a trash icon in its heading strip.
- Renaming happens from the note's own menu, not the list.
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

## Note colours

- **Default** (for new notes): the COSMIC theme's own look — its background, text
  and line colours, following light and dark mode.
- Classic sticky-note yellow, plus colours people know from Linux: Pop cyan, Ubuntu orange, Debian red, Fedora blue, openSUSE green,
  Arch blue, Manjaro teal-green and Mint green.
- Any other colour fills a note's paper and its card in the notes list, exactly as it is,
  in light and dark mode alike — no tints or shades.
- The dotted lines are a darker shade of the note's colour.
- In the notes list, a coloured card's heading strip is a deeper shade of its colour
  that keeps the colour's saturation (yellow deepens to gold, not olive). The
  card's preview and time (right-aligned, below a faint line in the text colour with
  8 px above and below it) use the note's text colour; the strip's title and icons
  pick dark or white against the darker strip.
- The title bar always keeps the COSMIC theme's colours — its background, the note's
  name, Settings and the window buttons — whatever the note's colour.
- With COSMIC's frosted glass turned on, the title bar is translucent and blurred
  like other COSMIC apps; the note's paper always stays solid.
- The note's text is dark or white, whichever reads better on its colour (at least
  4.5:1 contrast), whatever COSMIC's light or dark mode.
- The colour dialog shows the swatches three to a row.

## Window title bar

- Fleck draws each note window's title bar itself, in the same style as the notes
  list's: a **Settings** button on the left (COSMIC's text-button style: a grey label at rest; on hover a background, and the label turns the accent colour; its menu opens flush with the button's left edge), the note's name, then
  the window buttons COSMIC is set to show.
- Drag the bar to move the window; double-click it to maximise.

## Inside a note

- Just the note text. The menu and name live in the title bar.
- A note's name is its first line of text, until it is renamed from the note's
  menu or the notes list.
- **The note menu:**
  - **Rename** — a small "Rename note" dialog over the note, with Save and
    Cancel. Enter saves.
  - **Colour** — a "Note colour" dialog with a swatch for each colour, the
    current one outlined in the accent colour. Clicking a swatch recolours the note
    and closes the dialog; Cancel leaves it as it was.
  - **Delete** — the same "Delete note?" confirmation as the list. If no
    other window is open, the notes list opens afterwards.
  - **Notes** — opens the notes list, or brings it forward. The note stays open.
  - **Theme** — System, Light or Dark, for every Fleck window. System follows
    COSMIC. Remembered between runs.
  - Picking anything, pressing the settings button again, or clicking in the note closes it.
- The note text has no border.
- The note text is ruled like lined paper: evenly spaced dotted lines, one per
  line of text, filling the window whether or not there is text on them.
- Pressing Enter moves the cursor down onto the next line.
- Resizing the window fills the new space with lines.
- Text must sit on the lines, and the lines scroll with the text.

## Images in a note

- **Putting one in:** paste it (Ctrl+V), or drag image files onto the note. Either way
  the image lands where the cursor is. Pasting text still pastes text, and a dropped
  file that isn't an image Fleck can show is ignored.
- Drops come through COSMIC's own drag-and-drop, so they work on Wayland; the window
  toolkit's file-drop event is X11-only.
- **In the text:** the image shows as its own line — a picture icon and the file
  name — in the note's own colours. It is ordinary text, so it moves with the
  paragraph around it, and cut, paste, undo and search treat it like any other line.
- **Under the note:** a row of thumbnails, in the order the images appear in the text.
  It is only there when the note has images.
- **Opening one:** clicking a thumbnail puts the cursor on that image's line in the
  text and opens the picture full size over the note, with **Copy**, **Delete** and a
  close button in the dialog's top-right corner.
- **Copying:** **Copy** puts the picture back on the clipboard, ready to paste
  anywhere else.
- **Deleting:** **Delete** removes the image's line from the text and its file from disk.
- **On disk:** images are saved beside the note, in a folder named after it, and the
  note's text holds an ordinary Markdown link (`![](picture.png)`). Notes stay readable
  in any Markdown editor, and deleting a note deletes its images with it.
- **Not doing:** cropping, resizing, or drawing on images. Fleck's editor is plain text,
  so images cannot sit truly between two lines — the line plus its thumbnail is how
  their place is shown.

## Look

- Mostly theme defaults for now; the full design pass comes later.
- Working in multiples of 8.
- Notes list sizes, fixed rather than following COSMIC's roundness setting:
  - Content panel: 8 px padding on all four sides.
  - Search bar: 8 px padding left and right, 4 px corner radius.
  - Cards: 4 px corner radius, 8 px between cards.
  - Card heading: a darker strip across the top of the card, edge to edge, with
    the title on the left and the delete icon on the right of the same row.
    8 px padding inside it. Darker is derived from the card's theme colour.
  - 8 px between the heading strip and the card's content.
  - Card content (preview and time): 8 px padding, including top and bottom.
  - The delete icon is 8 px from the
    card's right edge, in line with the search bar's magnifier.
  - The delete icon shows no background, at rest or on hover. On hover the trash
    fills in solid in a deep red (#C01C28), the same in light and dark mode.
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
- Dialogs are modal: clicking outside does not dismiss them, as in other COSMIC apps.
  Escape closes a dialog, and a note's Settings menu.

## Icons

- Fleck uses its own bundled icons from **Iconoir** (MIT), not the system icon theme,
  so they look the same whatever COSMIC's icon theme is set to.
- **Stroke 2** for `page-edit` (panel icon).
- **Stroke 1.5** for `search` and `trash` (delete).
- Icons follow the theme's text colour, so they work in light and dark mode.
- Fleck's own app icon, for the panel list and the app library, comes with the design pass.

