---
title: Configuração
description: Cada chave de ~/.hark/config.toml, com seu padrão.
sidebar:
  order: 1
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: c172e07d0050
---

Tudo vive em `~/.hark/config.toml`. Toda chave é opcional; o arquivo abaixo é o conjunto completo com seus padrões. Edições feitas pelas Configurações preservam seus comentários e formatação.

```toml
# --- o agente ------------------------------------------------------------
claude_bin       = "claude"      # nome do binário, ou um caminho absoluto
model            = "sonnet"      # o que --model recebe por padrão
projects_dir     = "~/.claude/projects"   # onde o CLI guarda os logs de sessão

[agent]
plugin = "claude"                # qual agente abre chats NOVOS
ask    = ""                      # qual agente responde a faixa barata; vazio segue plugin

[models]                         # as quatro camadas, nos nomes globais (Claude)
light    = "haiku"
standard = "sonnet"
heavy    = "sonnet"
max      = "opus"
# inicio = "sonnet"               # o modelo por trás do Início; light se não definido

# --- outros agentes ------------------------------------------------------------
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
# GEMINI_API_KEY = "…"           # nunca vai para o log

[agents.claude-acp]
enabled = false                  # vem desligado: mesma assinatura do claude

# --- voz ----------------------------------------------------------------
language      = "pt"             # o que o MICROFONE espera; "auto" detecta a cada fala
ui_language   = "pt"             # o que a TELA mostra: "pt" | "en"
voice         = "Luciana"        # uma voz do say do macOS
whisper_model = ""               # vazio = <data dir>/models/ggml-small.bin
hotkey        = "cmd+shift+space"
vocab         = ["webhook", "pull request", "deploy", "branch", "commit"]

# --- o que uma pergunta pode ver ----------------------------------------------
repos                = []
hours_back           = 36
prompt_budget_chars  = 12000

# --- áreas de foco, estilo kubectl -------------------------------------------
default_context = "all"
[contexts.side-projects]
match_cwd = ["~/Projects/side"]
repos     = ["~/Projects/side/api"]

# --- workers --------------------------------------------------------------
worker_budget_usd = 5.0          # teto por TURNO, mantido pelo Hark; 0 = desligado
worker_max_turns  = 0
worker_mode       = ""           # "" | "manual" | "acceptEdits" | "plan" | "auto" | "bypass"
batch_messages    = false

# --- atualizações ----------------------------------------------------------------
auto_update = true

# --- o assistente --------------------------------------------------------
assistant_name = "Hark"
theme          = "hark"          # "hark" | "dracula"

# --- economizadores de token da comunidade, por processo de worker ---------------------------
[assist]
ponytail  = "full"
caveman   = "off"
tokensave = "off"
```

## As chaves que vale entender

**`language = "auto"`** deixa o modelo de fala detectar o idioma de cada fala; um código fixo é mais preciso em falas muito curtas, então continua como padrão.

**`language` vs `ui_language`.** Separadas de propósito: `language` ajusta o modelo de fala; `ui_language` escolhe o idioma em que o Hark escreve e fala, inclusive o que o agente é instruído a responder. Nenhuma das duas restringe o que o Hark entende.

**`worker_budget_usd`.** A proteção, por turno. O Hark precifica cada chamada de um turno em andamento conforme o stream relata e corta o turno entre chamadas quando a estimativa passa do teto, então o excesso é de no máximo uma chamada. Não é o `--max-budget-usd` do próprio Claude Code, que conta o processo inteiro. `0` desliga o teto.

**`worker_mode`.** A postura de permissão com que os novos workers começam. Vazio significa manual. Nenhum modo, inclusive `bypass`, desliga o piso de produção; `bypass` em si é do Claude Code, em outro lugar ele abre accept-edits e avisa.

**`[agent]` e `[agents.<id>]`.** `plugin` escolhe o agente que abre chats novos; um chat existente retoma no agente que o criou. Cada `[agents.<id>]` é um agente: os embutidos vêm com seus comandos e você sobrescreve qualquer campo; um id novo adiciona qualquer agente ACP. `models` mapeia as camadas para os nomes de cada agente. `env` é onde um gateway vai (`ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` em uma segunda entrada `plugin = "claude"`); valores em `env` nunca vão para o log.

**`[contexts]`.** Áreas de foco por prefixo de cwd. `hark use <name>` troca a ativa.

**`auto_update`.** A única chamada de rede que o Hark faz por conta própria: um GET nas releases do GitHub. Um build novo baixa em segundo plano, sua assinatura minisign é verificada contra uma chave compilada no app, e então ele espera o seu clique.

**`[assist]`.** Variáveis de ambiente por processo para economizadores de token da comunidade, então o comportamento deles é definido por worker, nunca nas suas configurações globais.

## O arquivo de memória do assistente

O chat inicial guarda sua própria personalidade e aprendizados em `~/.hark/HARK.md`, espelhado no arquivo de memória que cada agente habilitado carrega sozinho (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`) ao lado. O modelo o edita por conta própria pelo fluxo normal de permissão; Configurações tem um botão para abri-lo.
