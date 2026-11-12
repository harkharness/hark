---
title: El contrato de plugin
description: Un vocabulario de eventos y una hoja de capacidades; lo no soportado se muestra, nunca se oculta.
sidebar:
  order: 3
source_hash: b6a601cc0036
---

Hark es una cabina. Lo que realmente habla con un modelo es un plugin, y la cabina solo lo conoce a través de un contrato pequeño en `crates/hark-agent`. Dos plugins hablan por todos los agentes hoy: Claude Code, de forma nativa, y el plugin ACP para cualquier agente que hable el Agent Client Protocol.

## El vocabulario de eventos

Sea lo que sea que imprima el backend, el plugin lo traduce a un conjunto fijo de eventos:

| Evento | Significado |
|---|---|
| `SessionStarted` | en qué sesión escribe este proceso, sus comandos slash, sus servidores MCP |
| `AssistantText` | prosa del agente, en streaming |
| `ToolUse` | se llamó a una herramienta |
| `ToolResult` | terminó, y si falló |
| `PermissionRequest` | el backend pregunta si una herramienta puede correr |
| `Result` | el turno terminó: uso de tokens por modelo, costo, estado de error |
| `RateLimit` | estado de la ventana de cuota, cuando el backend lo reporta |
| `BackgroundDone` | un comando que el agente dejó corriendo terminó |
| `Ignored` | todo lo demás, conservado para que el stream siga siendo auditable |

## La hoja de capacidades

Lo que el backend puede hacer de verdad: `resume`, `permissions`, `structured_output`, `cost_reporting`, `history`, `live_list`, `slash_commands`, `memory_file`, `shell_tools`, `fork`, `directive_mode`, `directive_model`, `directive_effort`. La regla es **lo no soportado se muestra, nunca se oculta**: una función que el plugin no tiene se queda en pantalla, desactivada, con la razón al pasar el cursor. `shell_tools` nombra la herramienta de shell para el piso de producción, así que `terraform apply` nunca puede aprobarse automáticamente, sea cual sea el agente que lo ejecute. Un agente ACP negocia su hoja en cada intercambio inicial.

Fíjate en lo que no está en el contrato: ningún formato de prompt, ningún nombre de modelo, ningún concepto de proveedor.

## Los plugins fuera de proceso son ACP

Un plugin de la comunidad es un agente ACP, en cualquier lenguaje, más una línea de registro. Los integrados siguen el [registro ACP](https://github.com/agentclientprotocol/registry) para saber cómo se distribuye e invoca cada uno, y la comprobación de versión lo lee.

## El recorrido completo

`docs/PLUGIN-CONTRACT.md` en el repositorio recorre el contrato a fondo: la línea de registro, los niveles, los gateways, lo que ACP da y lo que no, el traspaso entre agentes y el plan de publicar el contrato como crate.
