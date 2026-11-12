---
title: El CLI hark
description: El mismo núcleo, sin interfaz.
sidebar:
  order: 2
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: c0463c9679c8
---

El mismo núcleo, sin interfaz; útil para scripts, CI, o simplemente por costumbre de terminal:

```
hark listen                                   # el bucle de voz, en la terminal
hark hear                                     # transcribir una frase
hark setup                                    # descargar el modelo de voz
hark ask "what's pending today?"              # una pregunta barata, alimentada por un snapshot, en el agente que usaría la app
hark dispatch [--session <id>] "<instruction>"
hark ps [--clear]                             # workers en ejecución
hark spend [--day|--week|--project] [--rebuild] [--export csv|json]
hark index                                    # (re)indexar el historial de sesiones
hark sessions                                 # lo que hay en disco
hark rename <session-id> <name>               # nombrar un chat, en su propio historial
hark use <context> | hark contexts            # áreas de foco, al estilo kubectl
hark board                                    # el tablero, como texto
hark demands                                  # la lista del día
```

El CLI lo instala el mismo script que la app, en `~/.local/bin`.
