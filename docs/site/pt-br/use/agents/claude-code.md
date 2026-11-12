---
title: Claude Code
description: "O plugin nativo: o CLI claude com suas configurações, servidores MCP, arquivo de memória e histórico de sessões."
sidebar:
  order: 1
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 370d4ec70eb2
---

O Claude Code é o plugin nativo do Hark: o CLI `claude` com suas configurações completas, servidores MCP, skills, arquivo de memória e histórico de sessões em disco. É o agente padrão quando está instalado, e aquele com que a maior parte do Hark foi testada.

## O que você ganha

- **Seu histórico.** O Hark indexa as sessões que o CLI já escreve em `~/.claude/projects`, então um chat que você começou no terminal três semanas atrás está na barra lateral, retomável. `history = "all"` (o padrão) também oferece pastas em que o CLI trabalhou e que ainda não são projeto, um clique cada em Criar.
- **Sua configuração.** Servidores MCP, arquivo de memória, skills, configurações de permissão e hooks carregam exatamente como no seu terminal. Feche o Hark e seu fluxo de trabalho no terminal está onde você deixou.
- **Custos.** O Claude Code precifica seus turnos, então o livro-razão mostra dólares, não só tokens.
- **O medidor da assinatura.** As porcentagens das janelas de 5h e 7d vêm de uma ponte opcional: um wrapper de um clique em torno da linha de status do Claude Code que copia seu JSON para um arquivo que o Hark lê. Ele faz backup das suas configurações antes, não muda nada até você clicar, e desinstala limpo (Configurações › Custos).
- **O modo bypass** é do próprio Claude Code: nele os CLIs de produção são bloqueados de vez. Veja [Permissões](/docs/pt-br/use/window/permissions/).
- **O nome de um chat é do chat.** Renomear no Hark escreve a mesma linha `custom-title` que o `/rename` do CLI escreve, então `claude --resume`, as linhas do Hark e qualquer outro leitor mostram um nome só.

## Um gateway da empresa

Uma segunda entrada `[agents.<id>]` com `plugin = "claude"` e `ANTHROPIC_BASE_URL` e `ANTHROPIC_AUTH_TOKEN` no seu `env` roteia toda a experiência nativa por um gateway. Valores em `env` nunca vão para o log. Veja [Configuração](/docs/pt-br/use/reference/configuration/).

## Se o binário está em um lugar incomum

```toml
claude_bin = "/opt/homebrew/bin/claude"
```
