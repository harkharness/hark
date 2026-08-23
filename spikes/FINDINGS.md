# Spike findings

Empirical results against Claude Code CLI v2.1.220 (macOS, Apple Silicon).
These findings drive the `claude_cli` adapter design.

## Spike 1: driving `claude` over stream-json

### Baseline (fast mode, no tools)

```
claude -p <prompt> --model sonnet --output-format json \
  --json-schema '<schema>' --tools "" --setting-sources "" \
  --system-prompt '<short voice prompt>'
```

- Cost **$0.0089**, ~4.7s. Same prompt with default settings (project CLAUDE.md,
  memory, all tool definitions): **$0.23**. With tools executing: **$0.33+**.
- Conclusion: fast mode must stay lean; Hark pre-bakes context in Rust.
- `--json-schema` works with both `json` and `stream-json` output. The schema is
  enforced via a synthetic `StructuredOutput` tool call; the final `result` event
  carries the validated JSON string in `.result`.
- `--output-format stream-json` requires `--verbose` in `-p` mode.
- `--bare` requires `ANTHROPIC_API_KEY`: not usable (subscription-only rule).

### Interactive input

`--input-format stream-json` accepts user messages on stdin:

```json
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"..."}]}}
```

Multi-turn within one spawn (process stays alive after `result` and accepts the
next user message): verification below in "Multi-turn and images".

### Permissions over the wire (the key discovery)

Permission prompts only surface as protocol messages when the CLI is started with
the (hidden, not in `--help`) flag:

```
--permission-prompt-tool stdio
```

Then, for any tool call not covered by allow rules, the CLI emits on stdout:

```json
{"type":"control_request","request_id":"<uuid>",
 "request":{"subtype":"can_use_tool","tool_name":"Bash",
            "input":{"command":"echo ...","description":"..."}}}
```

The client answers on stdin:

```json
{"type":"control_response",
 "response":{"subtype":"success","request_id":"<uuid>",
             "response":{"behavior":"allow"}}}
```

Deny variant: `{"behavior":"deny","message":"User rejected this action from Hark."}`.
Verified both ways: allow executed the tool (file created, `permission_denials: []`);
deny blocked it (`permission_denials: 1`, file untouched, Claude acknowledged the
rejection in its answer). Without the flag, `--permission-mode manual` silently
denies (no request emitted).

Notes:
- Sandbox-safe commands (e.g. plain `echo`) auto-run even in `manual` mode without
  triggering a permission request.
- Client -> CLI `initialize` handshake works and returns capabilities (commands,
  agents, models, account, output styles):

  ```json
  {"type":"control_request","request_id":"init-1","request":{"subtype":"initialize"}}
  ```

  The CLI replies with `control_response` -> `.response.response` full capability
  object. Useful for the UI (model list, account) but NOT required for
  `can_use_tool` to work; the `--permission-prompt-tool stdio` flag is what arms it.

### Multi-turn and images

Verified in one spawned process (`spikes/02-multiturn-image.sh`):

1. First user message answered (`result`: "ready"); process stayed alive.
2. Second user message carried an image content block
   (`{"type":"image","source":{"type":"base64","media_type":"image/png","data":...}}`
   alongside text) and was answered based on the image content.

So M4's persistent conversation and screenshot paste both work over plain
stream-json stdin. No `--resume` needed within a live session.

### Resume (spike 3, for the dispatcher)

- `claude -p "<instruction>" --resume <sessionId>` CONTINUES the same session:
  same `session_id` in the result, same jsonl file appended (verified 8 -> 14
  lines, no new file). No fork unless `--fork-session` is passed.
- Cost on a small resumed session: ~$0.001 (prompt cache read).
- Resuming a session that is OPEN in an interactive terminal is untested and
  risky (two writers, confusing UX). Design decision: the dispatcher refuses
  to target sessions listed by `claude agents --json` and asks the user to
  close them or pick another session.

### Session listing

`claude agents --json` lists live sessions (pid, cwd, sessionId, name, status)
without a TTY. Good for the "what are you executing?" snapshot.

## Spike 2: whisper-rs pt-BR

Build: `whisper-rs 0.14`, feature `metal`, compiles clean on Apple Silicon.

Test audio: `say -v Luciana` (pt-BR) rendered to 16kHz mono wav via `afconvert`
(no microphone needed to test STT quality).

| Model | Phrase | Result |
|---|---|---|
| small (466MB) | "Quais são as pendências de hoje no projeto hark?" | perfect except "hark" -> "VOCUS" |
| small | "Abre a tarefa da migração do webhook e prepara o pull request." | "webhook" -> "e-block", "pull request" -> "pulo-request" |

- Latency (small, Metal): first run ~11s (Metal shader warmup), subsequent ~0.4s
  for ~4s of audio. Warmup must happen at app start, not on first utterance.
- `small` alone mangles English tech vocabulary inside pt-BR speech.

Retest with `initial_prompt` vocabulary biasing
("webhook, pull request, PR, deploy, Claude Code, branch, commit, ..."):

| Model + bias | test1 | test2 | Latency |
|---|---|---|---|
| small + initial_prompt | perfect ("HARK" correct) | perfect ("webhook", "pull request" correct) | ~0.3s |
| large-v3-turbo + initial_prompt | perfect | "webhook" -> "e-book" | ~1.1-1.5s |

**Decision: default model = `small` (466MB) with `initial_prompt` vocabulary bias.**
Beats turbo on accuracy for our domain AND is 4x faster. Model stays configurable;
the bias list should be user-extendable in config.
