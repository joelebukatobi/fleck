# Lista de notas
add-note = Añadir nota
search-notes = Buscar notas

# Cuánto tiempo hace que se editó una nota, en su tarjeta.
just-now = ahora mismo
minutes-ago = hace { $count } min
hours-ago = { $count ->
    [one] hace { $count } hora
   *[other] hace { $count } horas
}
days-ago = { $count ->
    [one] hace { $count } día
   *[other] hace { $count } días
}
weeks-ago = { $count ->
    [one] hace { $count } semana
   *[other] hace { $count } semanas
}

# Diálogo al abrir Fleck, cuando había notas abiertas la última vez.
restore-title = ¿Volver a abrir las notas?
restore-body = { $count ->
    [one] ¿Volver a abrir { $count } nota de la última vez?
   *[other] ¿Volver a abrir { $count } notas de la última vez?
}
restore-confirm = Abrir
restore-dismiss = No, gracias

# Confirmación al eliminar.
delete-title = ¿Eliminar la nota?
delete-body = Se eliminará «{ $name }». Esto no se puede deshacer.
delete-confirm = Eliminar
cancel = Cancelar

# Menú de la nota (el botón Ajustes arriba a la izquierda).
settings = Ajustes
edit-name = Cambiar nombre
change-colour = Color
delete-note = Eliminar
back-to-list = Notas
theme = Tema
theme-system = Del sistema
theme-light = Claro
theme-dark = Oscuro

# Diálogo para cambiar el nombre de una nota.
rename-title = Cambiar el nombre
rename-placeholder = Nombre de la nota
save = Guardar

# Diálogo de imagen, al pulsar una miniatura bajo la nota.
image-copy = Copiar
image-delete = Eliminar
image-missing = Esta imagen ya no está en el disco.

# Diálogo de color, desde el menú de la nota.
colour-title = Color de la nota
colour-default = Predeterminado
colour-yellow = Amarillo
colour-pop = Pop
colour-ubuntu = Ubuntu
colour-debian = Debian
colour-fedora = Fedora
colour-opensuse = openSUSE
colour-arch = Arch
colour-manjaro = Manjaro
colour-mint = Mint

# Recordatorios: la otra mitad de la ventana de la lista.
tab-notes = Notas
tab-reminders = Recordatorios
add-reminder = Añadir recordatorio
reminders-empty = Todavía no hay recordatorios.
reminder-untitled = Recordatorio
reminder-unreadable = No se puede leer la hora de este recordatorio.
due-today = Hoy a las { $time }
due-tomorrow = Mañana a las { $time }
due-weekday = { $weekday } a las { $time }
due-date = { $date } a las { $time }
repeat-once = Una vez
repeat-daily = Cada día
repeat-weekdays = De lunes a viernes
repeat-weekly = Cada semana
repeat-monthly = Cada mes

# Diálogo para crear o editar un recordatorio.
reminder-new-title = Nuevo recordatorio
reminder-edit-title = Editar el recordatorio
reminder-text = Recordatorio
reminder-date = Fecha
reminder-time = Hora
reminder-repeat = Repetir
reminder-invalid = Escribe la fecha como 2026-10-01 y la hora como 09:30.
remind-me = Recordármelo

# Cuánto falta para la hora de un recordatorio, en su formulario.
in-minutes = { $count ->
    [one] en { $count } minuto
   *[other] en { $count } minutos
}
in-hours = { $count ->
    [one] en { $count } hora
   *[other] en { $count } horas
}
in-days = { $count ->
    [one] en { $count } día
   *[other] en { $count } días
}
ago-minutes = { $count ->
    [one] hace { $count } minuto
   *[other] hace { $count } minutos
}
ago-hours = { $count ->
    [one] hace { $count } hora
   *[other] hace { $count } horas
}
ago-days = { $count ->
    [one] hace { $count } día
   *[other] hace { $count } días
}
reminder-description = Descripción
reminder-is-event = ¿Es un evento?
reminder-location = Lugar
reminder-done = Hecho { $when }
reminders-clear-done = Borrar los terminados
getting-speech-model = Descargando el modelo de voz, unos 142 MB
