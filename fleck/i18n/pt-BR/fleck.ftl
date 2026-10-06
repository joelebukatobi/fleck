# Lista de notas
add-note = Adicionar nota
search-notes = Pesquisar notas

# Quando a nota foi editada pela última vez, no cartão dela.
just-now = agora mesmo
minutes-ago = há { $count } min
hours-ago = { $count ->
    [one] há { $count } hora
   *[other] há { $count } horas
}
days-ago = { $count ->
    [one] há { $count } dia
   *[other] há { $count } dias
}
weeks-ago = { $count ->
    [one] há { $count } semana
   *[other] há { $count } semanas
}

# Diálogo ao abrir o Fleck, quando havia notas abertas na última vez.
restore-title = Reabrir as notas?
restore-body = { $count ->
    [one] Reabrir { $count } nota da última vez?
   *[other] Reabrir { $count } notas da última vez?
}
restore-confirm = Reabrir
restore-dismiss = Não, obrigado

# Confirmação de exclusão.
delete-title = Excluir a nota?
delete-body = “{ $name }” será excluída. Isso não pode ser desfeito.
delete-confirm = Excluir
cancel = Cancelar

# Menu da nota (o botão Ajustes no canto superior esquerdo).
settings = Ajustes
edit-name = Renomear
change-colour = Cor
delete-note = Excluir
back-to-list = Notas
theme = Tema
theme-system = Do sistema
theme-light = Claro
theme-dark = Escuro

# Diálogo para renomear uma nota.
rename-title = Renomear a nota
rename-placeholder = Nome da nota
save = Salvar

# Diálogo de imagem, a partir de uma miniatura abaixo da nota.
image-copy = Copiar
image-delete = Excluir
image-missing = Esta imagem não está mais no disco.

# Diálogo de cor, a partir do menu da nota.
colour-title = Cor da nota
colour-default = Padrão
colour-yellow = Amarelo
colour-pop = Pop
colour-ubuntu = Ubuntu
colour-debian = Debian
colour-fedora = Fedora
colour-opensuse = openSUSE
colour-arch = Arch
colour-manjaro = Manjaro
colour-mint = Mint

# Lembretes: a outra metade da janela da lista.
tab-notes = Notas
tab-reminders = Lembretes
add-reminder = Adicionar lembrete
reminders-empty = Nenhum lembrete ainda.
reminder-untitled = Lembrete
reminder-unreadable = Não é possível ler a hora deste lembrete.
due-today = Hoje às { $time }
due-tomorrow = Amanhã às { $time }
due-weekday = { $weekday } às { $time }
due-date = { $date } às { $time }
repeat-once = Uma vez
repeat-daily = Todos os dias
repeat-weekdays = Dias de semana
repeat-weekly = Toda semana
repeat-monthly = Todo mês

# Diálogo para criar ou editar um lembrete.
reminder-new-title = Novo lembrete
reminder-edit-title = Editar o lembrete
reminder-text = Lembrete
reminder-date = Data
reminder-time = Hora
reminder-repeat = Repetir
reminder-invalid = Digite a data como 2026-10-01 e a hora como 09:30.
remind-me = Lembrar-me

# Quanto falta para a hora do lembrete, no formulário dele.
in-minutes = { $count ->
    [one] em { $count } minuto
   *[other] em { $count } minutos
}
in-hours = { $count ->
    [one] em { $count } hora
   *[other] em { $count } horas
}
in-days = { $count ->
    [one] em { $count } dia
   *[other] em { $count } dias
}
ago-minutes = { $count ->
    [one] há { $count } minuto
   *[other] há { $count } minutos
}
ago-hours = { $count ->
    [one] há { $count } hora
   *[other] há { $count } horas
}
ago-days = { $count ->
    [one] há { $count } dia
   *[other] há { $count } dias
}
reminder-description = Descrição
reminder-is-event = Isto é um evento?
reminder-location = Local
reminder-done = Concluído { $when }
reminders-clear-done = Limpar os concluídos
getting-speech-model = Baixando o modelo de voz, cerca de 142 MB
