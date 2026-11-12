---
title: O contrato de plugin
description: Um vocabulário de eventos e uma ficha de capacidades; o não suportado é mostrado, nunca escondido.
sidebar:
  order: 3
source_hash: b6a601cc0036
---

O Hark é uma cabine. O que de fato fala com um modelo é um plugin, e a cabine só o conhece por um contrato pequeno em `crates/hark-agent`. Dois plugins falam por todo agente hoje: o Claude Code, nativamente, e o plugin ACP para qualquer agente que fala o Agent Client Protocol.

## O vocabulário de eventos

O que quer que o backend imprima, o plugin traduz em um conjunto fixo de eventos:

| Evento | Significado |
|---|---|
| `SessionStarted` | em qual sessão este processo escreve, seus comandos de barra, seus servidores MCP |
| `AssistantText` | prosa do agente, em stream |
| `ToolUse` | uma ferramenta foi chamada |
| `ToolResult` | ela terminou, e se falhou |
| `PermissionRequest` | o backend está perguntando se uma ferramenta pode rodar |
| `Result` | o turno terminou: uso de tokens por modelo, custo, estado de erro |
| `RateLimit` | estado da janela de cota, quando o backend relata uma |
| `BackgroundDone` | um comando que o agente deixou rodando terminou |
| `Ignored` | todo o resto, guardado para que o stream continue auditável |

## A ficha de capacidades

O que o backend consegue de fato fazer: `resume`, `permissions`, `structured_output`, `cost_reporting`, `history`, `live_list`, `slash_commands`, `memory_file`, `shell_tools`, `fork`, `directive_mode`, `directive_model`, `directive_effort`. A regra é **o não suportado é mostrado, nunca escondido**: uma funcionalidade que o plugin não tem fica na tela, desabilitada, com o motivo ao passar o mouse. `shell_tools` nomeia a ferramenta de shell para o piso de produção, então `terraform apply` nunca pode ser aprovado automaticamente, seja qual for o agente que o roda. Um agente ACP negocia sua ficha a cada handshake.

Repare no que não está no contrato: nenhum formato de prompt, nenhum nome de modelo, nenhum conceito de fornecedor.

## Plugins fora do processo são ACP

Um plugin da comunidade é um agente ACP, em qualquer linguagem, mais uma linha de registro. Os embutidos seguem o [registro ACP](https://github.com/agentclientprotocol/registry) em como cada um é distribuído e invocado, e a verificação de versão o lê.

## O passo a passo completo

`docs/PLUGIN-CONTRACT.md` no repositório percorre o contrato em profundidade: a linha de registro, camadas, gateways, o que o ACP dá e o que não dá, transferência entre agentes, e o plano de publicar o contrato como um crate.
