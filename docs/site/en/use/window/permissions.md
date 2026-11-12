---
title: Permissions
description: Inline cards you answer by click, keyboard or voice, and a floor that does not move.
sidebar:
  order: 4
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Every privileged tool call renders as an inline card in the thread: the tool, the payload, and allow / always / deny. Answer by click, by keyboard, or by voice from any window ("yes", "no"). Nothing blocks the rest of the interface: a permission waiting in one thread never freezes another.

## The hard floor

Commands that touch production (`kubectl apply`, `terraform apply`, `helm`, force pushes, destructive SQL, cloud deletes, package publishes) are never auto-approved, whichever agent runs them. Standing "always allow" rules are ignored for them, the card turns red, and in bypass mode those CLIs are blocked outright on Claude Code. That block is a Claude Code flag with no equivalent elsewhere, so on any other agent a bypass pick opens the thread in accept-edits instead and says so in the chat. Hark can be careless with your tokens; it is not allowed to be careless with your infrastructure.

## Modes

The permission pill beside the composer sets the posture new turns start in: manual (ask about everything), accept edits, plan, auto, bypass. `worker_mode` in the [configuration](/docs/en/use/reference/configuration/) sets the default. No mode disables the floor.
