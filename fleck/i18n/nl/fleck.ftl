# Notitielijst
add-note = Notitie toevoegen
search-notes = Notities zoeken

# Wanneer een notitie het laatst is bewerkt, op de kaart.
just-now = net nu
minutes-ago = { $count } min geleden
hours-ago = { $count ->
    [one] { $count } uur geleden
   *[other] { $count } uur geleden
}
days-ago = { $count ->
    [one] { $count } dag geleden
   *[other] { $count } dagen geleden
}
weeks-ago = { $count ->
    [one] { $count } week geleden
   *[other] { $count } weken geleden
}

# Dialoog bij het starten, als er notities open waren bij het afsluiten.
restore-title = Notities opnieuw openen?
restore-body = { $count ->
    [one] { $count } notitie van vorige keer opnieuw openen?
   *[other] { $count } notities van vorige keer opnieuw openen?
}
restore-confirm = Openen
restore-dismiss = Nee, bedankt

# Bevestiging bij verwijderen.
delete-title = Notitie verwijderen?
delete-body = ‘{ $name }’ wordt verwijderd. Dit kan niet ongedaan worden gemaakt.
delete-confirm = Verwijderen
cancel = Annuleren

# Menu van de notitie (de knop Instellingen linksboven).
settings = Instellingen
edit-name = Naam wijzigen
change-colour = Kleur
delete-note = Verwijderen
back-to-list = Notities
theme = Thema
theme-system = Systeem
theme-light = Licht
theme-dark = Donker

# Dialoog om een notitie een andere naam te geven.
rename-title = Naam van de notitie wijzigen
rename-placeholder = Naam van de notitie
save = Opslaan

# Afbeeldingsdialoog, via een miniatuur onder de notitie.
image-copy = Kopiëren
image-delete = Verwijderen
image-missing = Deze afbeelding staat niet meer op de schijf.

# Kleurdialoog, via het menu van de notitie.
colour-title = Kleur van de notitie
colour-default = Standaard
colour-yellow = Geel
colour-pop = Pop
colour-ubuntu = Ubuntu
colour-debian = Debian
colour-fedora = Fedora
colour-opensuse = openSUSE
colour-arch = Arch
colour-manjaro = Manjaro
colour-mint = Mint

# Herinneringen: de andere helft van het lijstvenster.
tab-notes = Notities
tab-reminders = Herinneringen
add-reminder = Herinnering toevoegen
reminders-empty = Nog geen herinneringen.
reminder-untitled = Herinnering
reminder-unreadable = De tijd van deze herinnering is onleesbaar.
due-today = Vandaag om { $time }
due-tomorrow = Morgen om { $time }
due-weekday = { $weekday } om { $time }
due-date = { $date } om { $time }
repeat-once = Eenmalig
repeat-daily = Elke dag
repeat-weekdays = Op werkdagen
repeat-weekly = Elke week
repeat-monthly = Elke maand

# Dialoog om een herinnering te maken of te wijzigen.
reminder-new-title = Nieuwe herinnering
reminder-edit-title = Herinnering wijzigen
reminder-text = Herinnering
reminder-date = Datum
reminder-time = Tijd
reminder-repeat = Herhalen
reminder-invalid = Voer de datum in als 2026-10-01 en de tijd als 09:30.
remind-me = Herinner mij

# Hoe ver de tijd van een herinnering weg is, in het formulier.
in-minutes = { $count ->
    [one] over { $count } minuut
   *[other] over { $count } minuten
}
in-hours = { $count ->
    [one] over { $count } uur
   *[other] over { $count } uur
}
in-days = { $count ->
    [one] over { $count } dag
   *[other] over { $count } dagen
}
ago-minutes = { $count ->
    [one] { $count } minuut geleden
   *[other] { $count } minuten geleden
}
ago-hours = { $count ->
    [one] { $count } uur geleden
   *[other] { $count } uur geleden
}
ago-days = { $count ->
    [one] { $count } dag geleden
   *[other] { $count } dagen geleden
}
reminder-description = Beschrijving
reminder-is-event = Is dit een gebeurtenis?
reminder-location = Locatie
reminder-done = Klaar { $when }
reminders-clear-done = Afgeronde wissen
getting-speech-model = Spraakmodel downloaden, ongeveer 142 MB
