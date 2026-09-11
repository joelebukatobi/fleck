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
