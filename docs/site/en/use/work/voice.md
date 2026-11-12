---
title: Voice
description: A global hotkey, what it heard and where it goes before anything runs, answers out loud.
sidebar:
  order: 3
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Press the hotkey (`cmd+shift+space` by default) anywhere, in any app. With the main window in front, the composer itself listens: the round draws over the text (waveform, what it heard, the target chip) and the window stays where you were reading. Anywhere else a floating HUD opens instead. Either way it stops on its own after a beat of silence, and the mic button on the composer starts the same round.

## What it heard, where it goes

Before anything runs, Hark shows you two things: what it heard and where it is going. Confirmation comes at one of two speeds:

- **The target is the chat you are already in**: a chip appears and 1.6 seconds of silence executes it.
- **Hark had to resolve the target**: it says the destination out loud and waits for a verdict. Say "yes" to go, "no" to cancel, or say a different instruction and it replaces the text and asks again. Ambiguity is never guessed away: you get numbered candidates and pick one ("the first one").

Answers are spoken back through the system voice while the details render on screen. Work sent by voice never switches what you are looking at: the chat it landed in pulses in the sidebar for a beat.

## Four spoken verbs

"Focus on billing" pins Início to that chat; "back" or "pause" releases it. In a focus, a sentence with no verb goes to the chat in focus. "Think harder" and "quick one" route a question up or down.

## When Hark speaks on its own

Never with the window in front: text does that job. Behind the window, a chat finishing its turn is said aloud, unless you are in a focus; then only the chat in focus ending is said, and everything else waits behind the bell.

## Vocabulary

`vocab` in the [configuration](/docs/en/use/reference/configuration/) biases the transcription toward your terms (project names, "pull request", "deploy"). The spoken grammar accepts Portuguese and English at the same time.
