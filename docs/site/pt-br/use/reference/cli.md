---
title: O CLI hark
description: O mesmo núcleo, sem interface.
sidebar:
  order: 2
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: c0463c9679c8
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
hark rename <session-id> <name>               # nomeia um chat, no próprio histórico dele
hark use <context> | hark contexts            # áreas de foco no estilo kubectl
hark board                                    # o quadro, como texto
hark demands                                  # a lista do dia
```

O CLI é instalado pelo mesmo script que o app, em `~/.local/bin`.
