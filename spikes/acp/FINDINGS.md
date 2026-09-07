# Spike ACP — o que o `gemini --acp` realmente fala

Gravado em 07/09/2026 nesta máquina, gemini-cli **0.46.0**, com
`spikes/acp/handshake.mjs` (jsonrpc 2.0 por stdio, uma linha por
mensagem). O handshake não chega ao modelo: custo zero.

## initialize — funciona, e responde mais do que a spec mínima

Enviado:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize",
 "params":{"protocolVersion":1,
           "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false}}}}
```

Recebido (campos que importam):

| campo | valor observado |
|---|---|
| `protocolVersion` | `1` |
| `agentInfo` | `{name: "gemini-cli", title: "Gemini CLI", version: "0.46.0"}` |
| `agentCapabilities.loadSession` | `true` |
| `agentCapabilities.promptCapabilities` | `{image: true, audio: true, embeddedContext: true}` |
| `agentCapabilities.mcpCapabilities` | `{http: true, sse: true}` |
| `authMethods` | 4: `oauth-personal`, `gemini-api-key`, `vertex-ai`, `gateway` |

Consequências para o plano:

- **`loadSession: true`** → `Capabilities.resume` do Hark é verdadeiro
  para o gemini. O historiador (F9.4) ainda é necessário para busca e
  mirror, mas retomar sessão é do agente.
- **`promptCapabilities.image: true`** → colar print num chat gemini
  funciona; o composer não precisa degradar.
- **`agentInfo.version`** é o que grava a versão nas fixtures (a
  disciplina do corpus de voz: fixture sem versão é fixture que mente).
- **`authMethods` é uma LISTA com id e descrição** — o `login_hint` do
  registry pode sair daqui em vez de ser texto nosso.

## session/new — o caminho de erro é o caminho comum

Enviado `{"method":"session/new","params":{"cwd":"…","mcpServers":[]}}`.
Recebido:

```json
{"jsonrpc":"2.0","id":2,
 "error":{"code":-32000,"message":"Gemini API key is missing or not configured."}}
```

Ou seja: **o gemini está instalado e não autenticado** nesta máquina.
Isso não é um caso de borda a tratar depois — é o primeiro estado que
qualquer usuário encontra, e prova que:

1. Detecção (`which gemini`) **não** implica utilizável. O catálogo
   precisa de um terceiro estado além de detectado/ausente:
   *presente, não autenticado*.
2. O erro chega como `error` de JSON-RPC no `session/new`, não como
   uma notificação `auth_required`. A tradução tem que reconhecer
   `-32000` com mensagem de auth e emitir o evento que a UI já sabe
   renderizar (`agent_auth: …` + `login_hint`).
3. Nunca dirigimos `authenticate` por conta própria: o card diz o que
   rodar (`gemini` e escolher o método) e o usuário decide.

## O que falta gravar (precisa de auth)

- `session/update` com `agent_message_chunk`, `thought`, `tool_call` /
  `tool_call_update`
- `session/request_permission` (e confirmar que existe opção
  `allow_once`, nunca `allow_always`)
- `usage`/`stop_reason` no fim do turno — o delta cumulativo do ledger
- `session/load` (resume)

Método: `HARK_ACP_TRACE=<arquivo>` no plugin grava tudo em jsonl e as
fixtures nascem do tráfego, não da spec.
