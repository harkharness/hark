---
title: O CLI hark
description: O mesmo núcleo, sem interface.
sidebar:
  order: 2
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 19eaad56b464
---

O mesmo núcleo, sem interface; útil para scripts, CI, ou só por hábito de terminal:

```
hark listen                                   # o laço de voz, no terminal
hark hear                                     # transcreve uma fala
hark setup                                    # baixa o modelo de fala
hark ask "o que está pendente hoje?"          # uma pergunta barata, alimentada por snapshot, no agente que o app usaria
hark dispatch [--session <id>] "<instruction>"
hark ps [--clear]                             # workers rodando
hark spend [--day|--week|--project] [--rebuild] [--export csv|json]
hark index                                    # (re)indexa o histórico de sessões
hark sessions                                 # o que está em disco
hark search [--any] [--project <name>] <words…>  # onde as palavras foram ditas, em qualquer sessão
hark show <session-id> [--tail <n>]           # uma sessão lida de volta como conversa
hark rename <session-id> <name>               # nomeia um chat, no próprio histórico dele
hark use <context> | hark contexts            # áreas de foco no estilo kubectl
hark board                                    # o quadro, como texto
hark demands                                  # a lista do dia
```

O CLI é instalado pelo mesmo script que o app, em `~/.local/bin`.

`hark search` procura no que foi dito de fato, por você e pelo agente, não só nos títulos: todas as palavras na mesma mensagem por padrão, `--any` para qualquer uma delas, uma "frase entre aspas" fica inteira. A primeira execução lê todas as sessões uma vez; depois responde na hora. `hark show` aceita os primeiros caracteres do id que o `search` imprime. Os dois são locais e não gastam tokens.
