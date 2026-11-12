---
title: First run
description: "The wizard: language, name and hotkey; an agent; the speech model; the microphone."
sidebar:
  order: 3
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Open `hark.app`. A wizard runs once and refuses to leave you with a half-working install:

1. **Language, name and hotkey.** What the microphone expects to hear, what the screen shows, what you want to call your assistant, and the global key that opens the mic from anywhere. All of it is editable later in Settings.
2. **Agent plugin.** Hark checks which agent CLIs are installed and logged in. Claude Code plugs in natively; Gemini CLI, Codex, Claude Code over ACP and any other Agent Client Protocol agent through the ACP plugin. The catalog shows each one, its install line, and whether your build is the one the ACP registry publishes. Nothing runs until at least one is detected.
3. **Speech model.** Downloads a whisper model (`small`, about 466 MB, the default; `large-v3-turbo` if you want more accuracy for the price of latency). The download is checksummed and resumable.
4. **Microphone test.** Records two seconds and shows you the transcription, which is also what triggers the macOS permission prompt, with context, before you need it in the middle of something.

Everything the wizard writes lands in `~/.hark/config.toml`, in plain text, with your comments preserved on later edits.

## Two languages, at the same time

Hark's spoken grammar accepts Portuguese and English simultaneously; you never pick one. "abre a board" and "open the board" both switch tabs; "sim" and "yes" both confirm; "a primeira" and "the first one" both pick the first candidate. Words that mean different things in each language are disambiguated rather than dropped: a bare "no" is a refusal, "no projeto webhook" addresses a target.

Two separate settings control the rest:

| Setting | What it decides |
|---|---|
| `language` | what the speech model expects to **hear**: a fixed code, or `auto` to detect each utterance |
| `ui_language` | what Hark **writes and says**: interface, local answers, warnings, and the language the agent is told to answer in |

They are separate on purpose: plenty of people speak Portuguese to an English interface, or the other way around.

## Where things live

| Path | What |
|---|---|
| `~/.hark/config.toml` | your configuration ([reference](/docs/en/use/reference/configuration/)) |
| `~/.hark/index.db` | session index and spend ledger |
| `~/.hark/models/` | speech models |
| `~/.hark/HARK.md` | the assistant's personality and learnings, mirrored into each agent's own memory file name beside it |
| `~/.hark/artifacts/<space>/` | the documentation tree: pages and the `index.json` that orders them |
| `~/.hark/state.json` | board, workers, active context, what each agent turned out to offer |
| `~/.hark/sessions/<agent>/<id>.jsonl` | Hark's own record of sessions on agents that keep none |
| `<project>/.hark/` | per-project journal, state and dispatch briefs |

None of it leaves your machine.
