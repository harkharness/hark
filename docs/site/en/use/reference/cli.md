---
title: The hark CLI
description: The same core, headless.
sidebar:
  order: 2
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

The same core, headless; useful for scripting, CI, or just a terminal habit:

```
hark listen                                   # the voice loop, in the terminal
hark hear                                     # transcribe one utterance
hark setup                                    # download the speech model
hark ask "what's pending today?"              # a cheap, snapshot-fed question, on the agent the app would use
hark dispatch [--session <id>] "<instruction>"
hark ps [--clear]                             # running workers
hark spend [--day|--week|--project] [--rebuild] [--export csv|json]
hark index                                    # (re)index session history
hark sessions                                 # what is on disk
hark rename <session-id> <name>               # name a chat, in its own history
hark use <context> | hark contexts            # kubectl-style focus areas
hark board                                    # the board, as text
hark demands                                  # the list of the day
```

The CLI is installed by the same script as the app, into `~/.local/bin`.
