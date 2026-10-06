# Elenco delle note
add-note = Aggiungi nota
search-notes = Cerca tra le note

# Quando una nota è stata modificata, sulla sua scheda.
just-now = adesso
minutes-ago = { $count } min fa
hours-ago = { $count ->
    [one] { $count } ora fa
   *[other] { $count } ore fa
}
days-ago = { $count ->
    [one] { $count } giorno fa
   *[other] { $count } giorni fa
}
weeks-ago = { $count ->
    [one] { $count } settimana fa
   *[other] { $count } settimane fa
}

# Finestra all'avvio, quando c'erano note aperte l'ultima volta.
restore-title = Riaprire le note?
restore-body = { $count ->
    [one] Riaprire { $count } nota dell'ultima volta?
   *[other] Riaprire { $count } note dell'ultima volta?
}
restore-confirm = Riapri
restore-dismiss = No, grazie

# Conferma di eliminazione.
delete-title = Eliminare la nota?
delete-body = «{ $name }» sarà eliminata. L'operazione è irreversibile.
delete-confirm = Elimina
cancel = Annulla

# Menu della nota (il pulsante Impostazioni in alto a sinistra).
settings = Impostazioni
edit-name = Rinomina
change-colour = Colore
delete-note = Elimina
back-to-list = Note
theme = Tema
theme-system = Sistema
theme-light = Chiaro
theme-dark = Scuro

# Finestra per rinominare una nota.
rename-title = Rinomina la nota
rename-placeholder = Nome della nota
save = Salva

# Finestra dell'immagine, da una miniatura sotto la nota.
image-copy = Copia
image-delete = Elimina
image-missing = Questa immagine non è più sul disco.

# Finestra del colore, dal menu della nota.
colour-title = Colore della nota
colour-default = Predefinito
colour-yellow = Giallo
colour-pop = Pop
colour-ubuntu = Ubuntu
colour-debian = Debian
colour-fedora = Fedora
colour-opensuse = openSUSE
colour-arch = Arch
colour-manjaro = Manjaro
colour-mint = Mint

# Promemoria: l'altra metà della finestra dell'elenco.
tab-notes = Note
tab-reminders = Promemoria
add-reminder = Aggiungi promemoria
reminders-empty = Ancora nessun promemoria.
reminder-untitled = Promemoria
reminder-unreadable = Non si riesce a leggere l'ora di questo promemoria.
due-today = Oggi alle { $time }
due-tomorrow = Domani alle { $time }
due-weekday = { $weekday } alle { $time }
due-date = { $date } alle { $time }
repeat-once = Una volta
repeat-daily = Ogni giorno
repeat-weekdays = Nei giorni lavorativi
repeat-weekly = Ogni settimana
repeat-monthly = Ogni mese

# Finestra per creare o modificare un promemoria.
reminder-new-title = Nuovo promemoria
reminder-edit-title = Modifica il promemoria
reminder-text = Promemoria
reminder-date = Data
reminder-time = Ora
reminder-repeat = Ripeti
reminder-invalid = Scrivi la data come 2026-10-01 e l'ora come 09:30.
remind-me = Ricordamelo

# Quanto manca all'ora del promemoria, nel suo modulo.
in-minutes = { $count ->
    [one] tra { $count } minuto
   *[other] tra { $count } minuti
}
in-hours = { $count ->
    [one] tra { $count } ora
   *[other] tra { $count } ore
}
in-days = { $count ->
    [one] tra { $count } giorno
   *[other] tra { $count } giorni
}
ago-minutes = { $count ->
    [one] { $count } minuto fa
   *[other] { $count } minuti fa
}
ago-hours = { $count ->
    [one] { $count } ora fa
   *[other] { $count } ore fa
}
ago-days = { $count ->
    [one] { $count } giorno fa
   *[other] { $count } giorni fa
}
reminder-description = Descrizione
reminder-is-event = È un evento?
reminder-location = Luogo
reminder-done = Fatto { $when }
reminders-clear-done = Cancella i completati
getting-speech-model = Scaricamento del modello vocale, circa 142 MB
