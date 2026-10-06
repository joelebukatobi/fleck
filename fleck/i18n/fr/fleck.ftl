# Liste des notes
add-note = Ajouter une note
search-notes = Rechercher des notes

# Date de la dernière modification, sur la carte de la note.
just-now = à l'instant
minutes-ago = il y a { $count } min
hours-ago = { $count ->
    [one] il y a { $count } heure
   *[other] il y a { $count } heures
}
days-ago = { $count ->
    [one] il y a { $count } jour
   *[other] il y a { $count } jours
}
weeks-ago = { $count ->
    [one] il y a { $count } semaine
   *[other] il y a { $count } semaines
}

# Dialogue au lancement, quand des notes étaient ouvertes la dernière fois.
restore-title = Rouvrir les notes ?
restore-body = { $count ->
    [one] Rouvrir { $count } note de la dernière fois ?
   *[other] Rouvrir { $count } notes de la dernière fois ?
}
restore-confirm = Rouvrir
restore-dismiss = Non merci

# Confirmation de suppression.
delete-title = Supprimer la note ?
delete-body = « { $name } » sera supprimée. C'est irréversible.
delete-confirm = Supprimer
cancel = Annuler

# Menu de la note (le bouton Réglages en haut à gauche).
settings = Réglages
edit-name = Renommer
change-colour = Couleur
delete-note = Supprimer
back-to-list = Notes
theme = Thème
theme-system = Système
theme-light = Clair
theme-dark = Sombre

# Dialogue pour renommer une note.
rename-title = Renommer la note
rename-placeholder = Nom de la note
save = Enregistrer

# Dialogue d'image, depuis une vignette sous la note.
image-copy = Copier
image-delete = Supprimer
image-missing = Cette image n'est plus sur le disque.

# Dialogue de couleur, depuis le menu de la note.
colour-title = Couleur de la note
colour-default = Par défaut
colour-yellow = Jaune
colour-pop = Pop
colour-ubuntu = Ubuntu
colour-debian = Debian
colour-fedora = Fedora
colour-opensuse = openSUSE
colour-arch = Arch
colour-manjaro = Manjaro
colour-mint = Mint

# Rappels : l'autre moitié de la fenêtre de la liste.
tab-notes = Notes
tab-reminders = Rappels
add-reminder = Ajouter un rappel
reminders-empty = Aucun rappel pour le moment.
reminder-untitled = Rappel
reminder-unreadable = L'heure de ce rappel est illisible.
due-today = Aujourd'hui à { $time }
due-tomorrow = Demain à { $time }
due-weekday = { $weekday } à { $time }
due-date = Le { $date } à { $time }
repeat-once = Une fois
repeat-daily = Chaque jour
repeat-weekdays = En semaine
repeat-weekly = Chaque semaine
repeat-monthly = Chaque mois

# Dialogue pour créer ou modifier un rappel.
reminder-new-title = Nouveau rappel
reminder-edit-title = Modifier le rappel
reminder-text = Rappel
reminder-date = Date
reminder-time = Heure
reminder-repeat = Répéter
reminder-invalid = Saisissez la date comme 2026-10-01 et l'heure comme 09:30.
remind-me = Me le rappeler

# Dans combien de temps le rappel se déclenche, dans son formulaire.
in-minutes = { $count ->
    [one] dans { $count } minute
   *[other] dans { $count } minutes
}
in-hours = { $count ->
    [one] dans { $count } heure
   *[other] dans { $count } heures
}
in-days = { $count ->
    [one] dans { $count } jour
   *[other] dans { $count } jours
}
ago-minutes = { $count ->
    [one] il y a { $count } minute
   *[other] il y a { $count } minutes
}
ago-hours = { $count ->
    [one] il y a { $count } heure
   *[other] il y a { $count } heures
}
ago-days = { $count ->
    [one] il y a { $count } jour
   *[other] il y a { $count } jours
}
reminder-description = Description
reminder-is-event = Est-ce un évènement ?
reminder-location = Lieu
reminder-done = Terminé { $when }
reminders-clear-done = Effacer les rappels terminés
getting-speech-model = Téléchargement du modèle vocal, environ 142 Mo
