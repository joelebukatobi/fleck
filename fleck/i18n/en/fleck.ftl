# Notes list
search-notes = Search notes

# How long ago a note was last edited, shown on its card.
just-now = just now
minutes-ago = { $count } min ago
hours-ago = { $count ->
    [one] { $count } hour ago
   *[other] { $count } hours ago
}
days-ago = { $count ->
    [one] { $count } day ago
   *[other] { $count } days ago
}
weeks-ago = { $count ->
    [one] { $count } week ago
   *[other] { $count } weeks ago
}

# Restore dialog, offered at launch when notes were open at last quit.
restore-title = Reopen notes?
restore-body = { $count ->
    [one] Reopen { $count } note from last time?
   *[other] Reopen { $count } notes from last time?
}
restore-confirm = Reopen
restore-dismiss = No thanks

# Delete confirmation.
delete-title = Delete note?
delete-body = "{ $name }" will be deleted. This can't be undone.
delete-confirm = Delete
cancel = Cancel

# Note window menu (the Settings button at the top-left of a note).
settings = Settings
edit-name = Rename
change-colour = Change colour
delete-note = Delete
back-to-list = Back to list
theme = Theme
theme-system = System
theme-light = Light
theme-dark = Dark

# Rename dialog, opened from a note's menu.
rename-title = Rename note
rename-placeholder = Note name
save = Save

# Colour dialog, opened from a note's menu.
colour-title = Note colour
colour-default = Default
colour-yellow = Yellow
colour-pop = Pop
colour-ubuntu = Ubuntu
colour-debian = Debian
colour-fedora = Fedora
colour-opensuse = openSUSE
colour-arch = Arch
colour-manjaro = Manjaro
colour-mint = Mint
