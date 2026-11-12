---
title: Gemini CLI, Codex y cualquier agente ACP
description: "El plugin ACP: un protocolo, todos los agentes que lo hablan."
sidebar:
  order: 2
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: b69ed59a0638
---

El [Agent Client Protocol](https://agentclientprotocol.com) es el estándar abierto de editor a agente (JSON-RPC sobre stdio, un intercambio de capacidades, permisos en el cable) contra el que publican Zed, JetBrains y Google. Hark lo habla una vez, y cada agente que lo habla se conecta: Gemini CLI (`gemini --acp`), Codex (`codex-acp`), Claude Code sobre ACP (`claude-agent-acp`, desactivado por defecto), DeepSeek, Kiro, Google Antigravity y cualquier otro agente ACP con una entrada `[agents.<id>]` en la configuración. Cada agente integrado sigue el [registro de agentes ACP](https://github.com/agentclientprotocol/registry) para saber cómo se distribuye y se invoca.

## Lo que la misma cabina da a todo agente ACP

- **Permisos, siempre.** La misma tarjeta en línea que con Claude Code, respondida por clic, teclado o voz. Permitir significa una vez; las reglas permanentes son de Hark.
- **Directivas que cruzan.** Las píldoras de modo, modelo y esfuerzo hablan al agente en sus propios ids, según lo que ofreció al inicio de la sesión. La píldora de modelo habla en niveles, y la línea de registro de cada agente dice cómo los llama (`haiku` para Claude, `gemini-2.5-flash-lite` para Gemini). Un control que un agente no ofrece se queda en pantalla, desactivado, con la razón.
- **Costos donde existen.** El adaptador de Claude pone precio a sus turnos; Codex reporta tokens; Gemini no reporta nada. Un turno al que nadie puso precio es `$ –` con la razón al pasar el cursor, nunca `$0.00`.
- **Un historial que Hark escribe.** Los agentes ACP no guardan archivo de sesión, así que Hark registra cada sesión que abre a medida que ocurren los turnos: se puede buscar, leer y retomar. El límite honesto: una sesión abierta fuera de Hark queda invisible.
- **El piso de producción.** `kubectl apply`, `terraform apply`, force push y compañía nunca se aprueban automáticamente, sea cual sea el agente que los ejecute y como sea que llame a su herramienta de shell.
- **Un techo.** El techo de dólares por turno es de Hark en todos los agentes.
- **Traspaso.** "Cambia este chat a Gemini" mueve el hilo: una sesión nueva en el otro agente, abierta con un brief local de cero tokens, en la misma tarjeta.

## Agregar un agente que no viene integrado

```toml
[agents.myagent]
cmd  = "myagent"
args = ["--acp"]
```

Aparece en el catálogo en el próximo arranque. Si el CLI que tu empresa permite no habla ACP, [abre un issue](https://github.com/harkharness/hark/issues) describiendo cómo transmite, si retoma sesiones y si reporta costo, o [escribe el plugin](/docs/es/build/plugin-contract/).

## Mantener los agentes al día

Detectado no es actualizado. Ajustes › Plugins compara la versión que reporta tu binario con la que publica el registro y muestra la línea de actualización. Un Gemini CLI viejo, por ejemplo, responde a cada sesión nueva con "This client is no longer supported"; eso es la actualización, no un problema de inicio de sesión.
