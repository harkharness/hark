---
title: A barra de sessão
description: Ambiente, contexto, conectores, saídas, mudanças, tarefas em segundo plano, custos e limites.
sidebar:
  order: 3
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: f98207961b40
---

A barra de sessão se lê como a do app do Claude Code: seções que dobram e lembram disso, cada uma com seu resumo enquanto dobrada.

| Seção | O que mostra |
|---|---|
| **Ambiente** | esta máquina, o projeto e sua pasta, a branch, o agente e seus ajustes |
| **Contexto** | a fatia da janela, e o que a preenche |
| **Conectores** | os servidores MCP que a sessão abriu, com seu estado; reconectar fica em Configurações › Plugins |
| **Saídas** | o que a sessão escreveu: uma página em `~/.hark/artifacts` pelo título e tipo, abrindo no painel de artefato; um markdown do workspace como doc, qualquer outro arquivo como arquivo, ambos abrindo na janela de arquivos |
| **Mudanças** | o git da sessão: cada arquivo mudado desde o último commit, as linhas do que você abre, um tique por arquivo que você já leu, e "commitar as mudanças", que entrega o pedido ao chat. O Hark não commita nada sozinho |
| **Tarefas em segundo plano** | comandos que o agente deixou rodando, e quantos terminaram |
| **Custos** e **Limites** | o que esta sessão custou, e as janelas da assinatura |

O diff vive aqui e em nenhum outro lugar: ele é por sessão.
