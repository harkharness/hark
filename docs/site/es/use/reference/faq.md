---
title: Preguntas frecuentes
description: Respuestas cortas primero.
sidebar:
  order: 4
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: af945f638f6c
---

## ¿Hark usa mi suscripción?

Sí, ese es el punto. Hark invoca los binarios de agente que ya autenticaste: `claude`, `gemini`, `codex-acp` y demás. No tiene API key y no puede cobrarte por separado.

## ¿Tengo que cambiar mi forma de trabajar?

No. Hark lee las sesiones que el CLI ya escribe en disco, respeta tus ajustes, archivo de memoria, servidores MCP y skills, y devuelve las mismas sesiones. Cierra Hark y tu flujo en la terminal sigue donde lo dejaste. A los agentes que no guardan archivo de sesión, Hark les escribe uno.

## ¿Es gratis? ¿Es de código abierto?

Sí y sí. Hark es MIT o Apache-2.0 a tu elección, está en [GitHub](https://github.com/harkharness/hark), y no cuesta nada más allá de las suscripciones de agentes que ya tienes.

## ¿Adónde van las páginas que escribe el agente?

A `~/.hark/artifacts/<space>/`, como archivos markdown, html o json que puedes abrir con cualquier cosa. Hark guarda un `index.json` a su lado y lo reconcilia con el disco en cada lectura.

## ¿Algo sale de mi máquina?

Una sola llamada propia: una comprobación de actualizaciones contra las releases de GitHub, que puedes desactivar. La voz corre en local. El índice y el libro de cuentas viven en tu carpeta de usuario. Sin telemetría, sin cuenta.

## ¿Windows o Linux?

Hoy, macOS 12 o más reciente, Apple Silicon e Intel. Linux y Windows son las próximas releases; [ayúdanos a probarlas](/docs/es/build/testing-other-agents-and-oss/).

## ¿Hark está afiliado a Anthropic, Google u OpenAI?

No. Hark es un proyecto independiente. Claude Code, Gemini CLI y Codex son marcas de sus respectivos dueños.
