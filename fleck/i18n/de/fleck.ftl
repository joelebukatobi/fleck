# Notizliste
add-note = Notiz hinzufügen
search-notes = Notizen durchsuchen

# Wann eine Notiz zuletzt bearbeitet wurde, auf ihrer Karte.
just-now = gerade eben
minutes-ago = vor { $count } Min.
hours-ago = { $count ->
    [one] vor { $count } Stunde
   *[other] vor { $count } Stunden
}
days-ago = { $count ->
    [one] vor { $count } Tag
   *[other] vor { $count } Tagen
}
weeks-ago = { $count ->
    [one] vor { $count } Woche
   *[other] vor { $count } Wochen
}

# Dialog beim Start, wenn beim letzten Beenden Notizen offen waren.
restore-title = Notizen wieder öffnen?
restore-body = { $count ->
    [one] { $count } Notiz von letztem Mal wieder öffnen?
   *[other] { $count } Notizen von letztem Mal wieder öffnen?
}
restore-confirm = Öffnen
restore-dismiss = Nein, danke

# Bestätigung vor dem Löschen.
delete-title = Notiz löschen?
delete-body = „{ $name }“ wird gelöscht. Das lässt sich nicht widerrufen.
delete-confirm = Löschen
cancel = Abbrechen

# Menü der Notiz (die Schaltfläche Einstellungen oben links).
settings = Einstellungen
edit-name = Umbenennen
change-colour = Farbe
delete-note = Löschen
back-to-list = Notizen
theme = Erscheinungsbild
theme-system = System
theme-light = Hell
theme-dark = Dunkel

# Dialog zum Umbenennen einer Notiz.
rename-title = Notiz umbenennen
rename-placeholder = Name der Notiz
save = Speichern

# Bilddialog, über ein Vorschaubild unter der Notiz.
image-copy = Kopieren
image-delete = Löschen
image-missing = Dieses Bild liegt nicht mehr auf der Festplatte.

# Farbdialog, über das Menü der Notiz.
colour-title = Farbe der Notiz
colour-default = Standard
colour-yellow = Gelb
colour-pop = Pop
colour-ubuntu = Ubuntu
colour-debian = Debian
colour-fedora = Fedora
colour-opensuse = openSUSE
colour-arch = Arch
colour-manjaro = Manjaro
colour-mint = Mint

# Erinnerungen: die andere Hälfte des Listenfensters.
tab-notes = Notizen
tab-reminders = Erinnerungen
add-reminder = Erinnerung hinzufügen
reminders-empty = Noch keine Erinnerungen.
reminder-untitled = Erinnerung
reminder-unreadable = Die Zeit dieser Erinnerung ist nicht lesbar.
due-today = Heute um { $time }
due-tomorrow = Morgen um { $time }
due-weekday = { $weekday } um { $time }
due-date = { $date } um { $time }
repeat-once = Einmal
repeat-daily = Täglich
repeat-weekdays = Wochentags
repeat-weekly = Wöchentlich
repeat-monthly = Monatlich

# Dialog zum Anlegen oder Bearbeiten einer Erinnerung.
reminder-new-title = Neue Erinnerung
reminder-edit-title = Erinnerung bearbeiten
reminder-text = Erinnerung
reminder-date = Datum
reminder-time = Zeit
reminder-repeat = Wiederholen
reminder-invalid = Datum als 2026-10-01 und Zeit als 09:30 eingeben.
remind-me = Erinnere mich

# Wie weit die Zeit einer Erinnerung entfernt ist, in ihrem Formular.
in-minutes = { $count ->
    [one] in { $count } Minute
   *[other] in { $count } Minuten
}
in-hours = { $count ->
    [one] in { $count } Stunde
   *[other] in { $count } Stunden
}
in-days = { $count ->
    [one] in { $count } Tag
   *[other] in { $count } Tagen
}
ago-minutes = { $count ->
    [one] vor { $count } Minute
   *[other] vor { $count } Minuten
}
ago-hours = { $count ->
    [one] vor { $count } Stunde
   *[other] vor { $count } Stunden
}
ago-days = { $count ->
    [one] vor { $count } Tag
   *[other] vor { $count } Tagen
}
reminder-description = Beschreibung
reminder-is-event = Ist das ein Termin?
reminder-location = Ort
reminder-done = Erledigt { $when }
reminders-clear-done = Erledigte entfernen
getting-speech-model = Spracherkennungsmodell wird geladen, etwa 142 MB
