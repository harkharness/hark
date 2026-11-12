---
title: Primera ejecución
description: "El asistente de configuración: idioma, nombre y atajo; un agente; el modelo de voz; el micrófono."
sidebar:
  order: 3
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: 8d4218edaf9e
---

Abre `hark.app`. Un asistente de configuración corre una sola vez y se niega a dejarte con una instalación a medias:

1. **Idioma, nombre y atajo.** Lo que el micrófono espera oír, lo que muestra la pantalla, cómo quieres llamar a tu asistente y la tecla global que abre el micrófono desde cualquier lugar. Todo se puede editar después en Ajustes.
2. **Plugin de agente.** Hark comprueba qué CLI de agentes están instalados y con sesión iniciada. Claude Code se conecta de forma nativa; Gemini CLI, Codex, Claude Code sobre ACP y cualquier otro agente Agent Client Protocol, a través del plugin ACP. El catálogo muestra cada uno, su línea de instalación y si tu build es el que publica el registro ACP. Nada corre hasta que se detecta al menos uno.
3. **Modelo de voz.** Descarga un modelo whisper (`small`, unos 466 MB, el predeterminado; `large-v3-turbo` si quieres más precisión a cambio de latencia). La descarga tiene checksum y se puede reanudar.
4. **Prueba de micrófono.** Graba dos segundos y te muestra la transcripción, que es también lo que dispara el aviso de permiso de macOS, con contexto, antes de que lo necesites en medio de algo.

Todo lo que el asistente escribe va a `~/.hark/config.toml`, en texto plano, con tus comentarios preservados en ediciones posteriores.

## Dos idiomas, al mismo tiempo

La gramática hablada de Hark acepta portugués e inglés a la vez; nunca eliges uno. "abre a board" y "open the board" cambian de pestaña; "sim" y "yes" confirman; "a primeira" y "the first one" eligen el primer candidato. Las palabras que significan cosas distintas en cada idioma se desambiguan en lugar de descartarse: un "no" a secas es una negativa, "no projeto webhook" apunta a un destino.

Dos ajustes separados controlan el resto:

| Ajuste | Qué decide |
|---|---|
| `language` | lo que el modelo de voz espera **oír**: un código fijo, o `auto` para detectar cada frase |
| `ui_language` | lo que Hark **escribe y dice**: la interfaz, las respuestas locales, los avisos y el idioma en que se le pide al agente que responda |

Están separados a propósito: mucha gente habla portugués a una interfaz en inglés, o al revés.

## Dónde vive cada cosa

| Ruta | Qué |
|---|---|
| `~/.hark/config.toml` | tu configuración ([referencia](/docs/es/use/reference/configuration/)) |
| `~/.hark/index.db` | índice de sesiones y libro de gastos |
| `~/.hark/models/` | modelos de voz |
| `~/.hark/HARK.md` | la personalidad y los aprendizajes del asistente, replicados a su lado en el archivo de memoria de cada agente, con el nombre que cada uno usa |
| `~/.hark/artifacts/<space>/` | el árbol de documentación: las páginas y el `index.json` que las ordena |
| `~/.hark/state.json` | tablero, workers, contexto activo, lo que cada agente resultó ofrecer |
| `~/.hark/sessions/<agent>/<id>.jsonl` | el registro propio de Hark de las sesiones en agentes que no guardan ninguno |
| `<project>/.hark/` | diario, estado y briefs de despacho por proyecto |

Nada de esto sale de tu máquina.
