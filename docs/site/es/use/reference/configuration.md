---
title: Configuración
description: Cada clave de ~/.hark/config.toml, con su valor predeterminado.
sidebar:
  order: 1
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: c172e07d0050
---

Todo vive en `~/.hark/config.toml`. Cada clave es opcional; el archivo de abajo es el conjunto completo con sus valores predeterminados. Las ediciones hechas desde Ajustes preservan tus comentarios y el formato.

```toml
# --- el agente ------------------------------------------------------------
claude_bin       = "claude"      # nombre del binario, o una ruta absoluta
model            = "sonnet"      # lo que --model recibe por defecto
projects_dir     = "~/.claude/projects"   # dónde el CLI guarda los logs de sesión

[agent]
plugin = "claude"                # qué agente abre los chats NUEVOS
ask    = ""                      # qué agente responde el carril barato; vacío sigue a plugin

[models]                         # los cuatro niveles, en los nombres globales (de Claude)
light    = "haiku"
standard = "sonnet"
heavy    = "sonnet"
max      = "opus"
# inicio = "sonnet"               # el modelo detrás de Início; light si no se define

# --- otros agentes ---------------------------------------------------------
[agents.claude]
history  = "all"                 # "all" | "projects" | "off"

[agents.gemini]
enabled     = true
cmd         = "gemini"
args        = ["--acp"]
memory_file = "GEMINI.md"
login_hint  = "gemini"
registry    = "gemini"
[agents.gemini.models]
light    = "gemini-2.5-flash-lite"
standard = "gemini-2.5-flash"
heavy    = "gemini-2.5-pro"
max      = "gemini-2.5-pro"
[agents.gemini.env]
# GEMINI_API_KEY = "…"           # nunca se registra en logs

[agents.claude-acp]
enabled = false                  # desactivado por defecto: la misma suscripción que claude

# --- voz ------------------------------------------------------------------
language      = "pt"             # lo que el MICRÓFONO espera; "auto" detecta por frase
ui_language   = "pt"             # lo que la PANTALLA muestra: "pt" | "en"
voice         = "Luciana"        # una voz de say de macOS
whisper_model = ""               # vacío = <data dir>/models/ggml-small.bin
hotkey        = "cmd+shift+space"
vocab         = ["webhook", "pull request", "deploy", "branch", "commit"]

# --- lo que una pregunta puede ver ----------------------------------------
repos                = []
hours_back           = 36
prompt_budget_chars  = 12000

# --- áreas de foco, al estilo kubectl -------------------------------------
default_context = "all"
[contexts.side-projects]
match_cwd = ["~/Projects/side"]
repos     = ["~/Projects/side/api"]

# --- workers --------------------------------------------------------------
worker_budget_usd = 5.0          # techo por TURNO, sostenido por Hark; 0 = apagado
worker_max_turns  = 0
worker_mode       = ""           # "" | "manual" | "acceptEdits" | "plan" | "auto" | "bypass"
batch_messages    = false

# --- actualizaciones --------------------------------------------------------
auto_update = true

# --- el asistente ---------------------------------------------------------
assistant_name = "Hark"
theme          = "hark"          # "hark" | "dracula"

# --- ahorradores de tokens de la comunidad, por proceso worker ------------
[assist]
ponytail  = "full"
caveman   = "off"
tokensave = "off"
```

## Las claves que vale la pena entender

**`language = "auto"`** deja que el modelo de voz detecte el idioma de cada frase; un código fijo es más preciso en frases muy cortas, por eso sigue siendo el predeterminado.

**`language` vs `ui_language`.** Separados a propósito: `language` ajusta el modelo de voz; `ui_language` elige el idioma en que Hark escribe y habla, incluido el que se le pide al agente que use al responder. Ninguno restringe lo que Hark entiende.

**`worker_budget_usd`.** La barrera, por turno. Hark pone precio a cada llamada de un turno en curso a medida que el stream la reporta y corta el turno entre llamadas cuando la estimación pasa el techo, así que el exceso es como mucho una llamada. No es el `--max-budget-usd` propio de Claude Code, que cuenta el proceso entero. `0` desactiva el techo.

**`worker_mode`.** La postura de permisos con la que arrancan los workers nuevos. Vacío significa manual. Ningún modo, incluido `bypass`, desactiva el piso de producción; `bypass` en sí es de Claude Code, en otros agentes abre accept-edits y lo dice.

**`[agent]` y `[agents.<id>]`.** `plugin` elige el agente que abre los chats nuevos; un chat existente se retoma en el agente que lo creó. Cada `[agents.<id>]` es un agente: los integrados vienen con sus comandos y tú sobrescribes cualquier campo; un id nuevo agrega cualquier agente ACP. `models` mapea los niveles a los nombres de cada agente. `env` es donde va un gateway (`ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` en una segunda entrada `plugin = "claude"`); los valores de `env` nunca se registran en logs.

**`[contexts]`.** Áreas de foco indexadas por prefijo de cwd. `hark use <name>` cambia la activa.

**`auto_update`.** La única llamada de red que Hark hace por su cuenta: un GET a las releases de GitHub. Un build nuevo se descarga en segundo plano, su firma minisign se verifica contra una clave compilada en la app, y luego espera tu clic.

**`[assist]`.** Variables de entorno por proceso para los ahorradores de tokens de la comunidad, de modo que su comportamiento se fija por worker, nunca en tus ajustes globales.

## El archivo de memoria del asistente

El chat de inicio guarda su propia personalidad y sus aprendizajes en `~/.hark/HARK.md`, replicados a su lado en el archivo de memoria que cada agente habilitado carga automáticamente (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`). El modelo lo edita por sí mismo a través del flujo normal de permisos; Ajustes tiene un botón para abrirlo.
