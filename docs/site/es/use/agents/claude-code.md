---
title: Claude Code
description: "El plugin nativo: el CLI claude con tus ajustes, servidores MCP, archivo de memoria e historial de sesiones."
sidebar:
  order: 1
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: 370d4ec70eb2
---

Claude Code es el plugin nativo de Hark: el CLI `claude` con todos tus ajustes, servidores MCP, skills, archivo de memoria e historial de sesiones en disco. Es el agente predeterminado cuando está instalado, y con el que se probó la mayor parte de Hark.

## Lo que obtienes

- **Tu historial.** Hark indexa las sesiones que el CLI ya escribe bajo `~/.claude/projects`, así que un chat que empezaste en la terminal hace tres semanas está en la barra lateral, listo para retomar. `history = "all"` (el valor predeterminado) también ofrece las carpetas en las que el CLI trabajó y que todavía no son proyecto, un clic cada una en Criar.
- **Tu configuración.** Servidores MCP, archivo de memoria, skills, ajustes de permisos y hooks cargan exactamente como en tu terminal. Cierra Hark y tu flujo en la terminal sigue donde lo dejaste.
- **Costos.** Claude Code pone precio a sus turnos, así que el libro de cuentas muestra dólares, no solo tokens.
- **El medidor de la suscripción.** Los porcentajes de las ventanas de 5h y 7d vienen de un puente opcional: un wrapper de un clic sobre la línea de estado de Claude Code que copia su JSON a un archivo que Hark lee. Primero respalda tus ajustes, no cambia nada hasta que haces clic y se desinstala limpiamente (Ajustes › Costos).
- **El modo bypass** es propio de Claude Code: en él, los CLI de producción se bloquean por completo. Ver [Permisos](/docs/es/use/window/permissions/).
- **El nombre de un chat es del chat.** Renombrar en Hark escribe la misma línea `custom-title` que escribe el `/rename` del CLI, así que `claude --resume`, las filas de Hark y cualquier otro lector muestran un solo nombre.

## Un gateway de empresa

Una segunda entrada `[agents.<id>]` con `plugin = "claude"` y `ANTHROPIC_BASE_URL` y `ANTHROPIC_AUTH_TOKEN` en su `env` enruta toda la experiencia nativa a través de un gateway. Los valores de `env` nunca se registran en logs. Ver [Configuración](/docs/es/use/reference/configuration/).

## Si el binario vive en un lugar inusual

```toml
claude_bin = "/opt/homebrew/bin/claude"
```
