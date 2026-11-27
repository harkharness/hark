---
title: A barra de sessão
description: Ambiente, contexto, conectores, saídas, mudanças, tarefas em segundo plano, custos e limites.
sidebar:
  order: 3
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 0a8baeeaf648
---

A barra de sessão se lê como a do app do Claude Code: a coluna do próprio chat, entre a conversa e os painéis do chat, com seções que dobram e lembram disso, cada uma com seu resumo enquanto dobrada. Ela não é um dos painéis do chat, então o terminal e os arquivos nunca dividem altura com ela. Quando a coluna do chat fica estreita demais para a conversa e a barra, porque a janela é pequena ou um painel foi arrastado para ficar largo, a barra se esconde sozinha; o botão dela no topo do chat a mostra por cima da página para uma olhada, e um clique fora ou Esc a guarda. Com espaço, o mesmo botão guarda a barra ou a traz de volta, e isso fica lembrado. Onde não há chat aberto não há barra.

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
