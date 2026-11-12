---
title: Qué es Hark
description: Un escritorio de código abierto para los CLI de agentes de código que ya pagas, con voz cuando la quieres.
sidebar:
  order: 1
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: b6ff161edc8c
---

Hark es una app de escritorio para los CLI de agentes de código que ya pagas: Claude Code de forma nativa; Gemini CLI, Codex y cualquier agente [Agent Client Protocol](https://agentclientprotocol.com) a través de un solo plugin. Una ventana reúne todos los proyectos. Cada chat se abre junto a una terminal, los archivos del proyecto, un navegador y el panel de la sesión; los agentes producen documentos, hojas de cálculo, pizarras y dashboards que editas, comentas y devuelves; un tablero sigue todas las tareas, cada tarjeta una sesión; el costo de cada chat queda a la vista. Pregunta en voz alta y escucha la respuesta cuando quieras; escribe cuando no.

## Lo que nunca hace

Hark nunca llama a la API de un modelo a tus espaldas. Maneja los CLI de agentes que ya tienes instalados y autenticados, así que cada token sale de la suscripción que ya pagas. Sin API key, sin segunda factura, sin telemetría, sin cuenta. El modelo de voz corre en local; el audio nunca sale de la máquina.

## Lo que necesitas

- macOS 12 o más reciente, Apple Silicon o Intel. Linux y Windows son las próximas releases.
- Al menos un CLI de agente compatible, instalado y con sesión iniciada. El [CLI de Claude Code](https://code.claude.com) da la experiencia más completa; [Gemini CLI](https://github.com/google-gemini/gemini-cli), Codex (`@agentclientprotocol/codex-acp`) o cualquier otro agente ACP funciona a través del plugin ACP.
- Unos 500 MB de disco para el modelo de voz.

## Adónde ir después

- [Instalar](/docs/es/use/start/install/) y [Primera ejecución](/docs/es/use/start/first-run/).
- [La ventana, panel por panel](/docs/es/use/window/home-and-chat/).
- [Artefactos: seis tipos de página](/docs/es/use/artifacts/kinds/).
- ¿Algo se rompió? [Solución de problemas](/docs/es/use/reference/troubleshooting/).
