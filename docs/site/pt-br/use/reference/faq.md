---
title: FAQ
description: Respostas curtas primeiro.
sidebar:
  order: 4
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: af945f638f6c
---

## O Hark usa a minha assinatura?

Sim, esse é o ponto. O Hark chama os binários de agente que você já autenticou: `claude`, `gemini`, `codex-acp` e assim por diante. Ele não tem chave de API e não consegue cobrar você separadamente.

## Preciso mudar como eu trabalho?

Não. O Hark lê as sessões que o CLI já escreve em disco, respeita suas configurações, arquivo de memória, servidores MCP e skills, e devolve as mesmas sessões. Feche o Hark e seu fluxo de trabalho no terminal está onde você deixou. Agentes que não guardam arquivo de sessão ganham um escrito pelo Hark.

## É de graça? É open source?

Sim e sim. O Hark é MIT ou Apache-2.0, à sua escolha, no [GitHub](https://github.com/harkharness/hark), e não custa nada além das assinaturas de agente que você já tem.

## Para onde vão as páginas que o agente escreve?

Para `~/.hark/artifacts/<space>/`, como arquivos markdown, html ou json que você abre com qualquer coisa. O Hark mantém um `index.json` ao lado e o reconcilia com o disco a cada leitura.

## Alguma coisa sai da minha máquina?

Uma chamada própria: uma verificação de atualização nas releases do GitHub, que você pode desligar. A fala roda localmente. O índice e o livro-razão vivem na sua pasta de usuário. Sem telemetria, sem cadastro.

## Windows ou Linux?

macOS 12 ou mais novo hoje, Apple Silicon e Intel. Linux e Windows são as próximas releases; [ajude a testá-las](/docs/pt-br/build/testing-other-agents-and-oss/).

## O Hark é afiliado à Anthropic, ao Google ou à OpenAI?

Não. O Hark é um projeto independente. Claude Code, Gemini CLI e Codex são marcas de seus respectivos donos.
