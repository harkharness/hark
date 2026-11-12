---
title: O que é o Hark
description: Um desktop open source para os CLIs de agentes de código que você já paga, com voz quando você quiser.
sidebar:
  order: 1
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 5514496ae2e6
---

O Hark é um desktop para os CLIs de agentes de código que você já paga: Claude Code nativamente; Gemini CLI, Codex e qualquer agente [Agent Client Protocol](https://agentclientprotocol.com) por um único plugin. Uma janela reúne todos os projetos: chats com um terminal de verdade, os arquivos, um navegador e uma barra de sessão ao lado; documentos, planilhas, quadros brancos e dashboards que os agentes escrevem e você edita, comenta e devolve; um quadro em que cada cartão é uma sessão; um livro-razão de cada dólar. Pergunte em voz alta e ouça a resposta quando quiser, digite quando não quiser.

## O que ele nunca faz

O Hark nunca chama uma API de modelo pelas suas costas. Ele aciona os CLIs de agente que você já tem instalados e autenticados, então cada token sai da assinatura que você já paga. Sem chave de API, sem segunda conta, sem telemetria, sem cadastro. O modelo de fala roda localmente; o áudio nunca sai da máquina.

## O que você precisa

- macOS 12 ou mais novo, Apple Silicon ou Intel. Linux e Windows são as próximas releases.
- Pelo menos um CLI de agente suportado, instalado e logado. O [CLI do Claude Code](https://code.claude.com) tem a experiência mais completa; [Gemini CLI](https://github.com/google-gemini/gemini-cli), Codex (`@agentclientprotocol/codex-acp`) ou qualquer outro agente ACP funciona pelo plugin ACP.
- Cerca de 500 MB de disco para o modelo de fala.

## Para onde ir agora

- [Instalar](/docs/pt-br/use/start/install/) e [Primeira execução](/docs/pt-br/use/start/first-run/).
- [A janela, painel por painel](/docs/pt-br/use/window/home-and-chat/).
- [Artefatos: seis tipos de página](/docs/pt-br/use/artifacts/kinds/).
- Algo quebrou? [Solução de problemas](/docs/pt-br/use/reference/troubleshooting/).
