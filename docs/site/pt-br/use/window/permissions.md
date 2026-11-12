---
title: Permissões
description: Cartões inline que você responde por clique, teclado ou voz, e um piso que não se move.
sidebar:
  order: 4
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: f0fabf7fd5f1
---

Toda chamada de ferramenta privilegiada é renderizada como um cartão inline na conversa: a ferramenta, o payload, e permitir / sempre / negar. Responda por clique, pelo teclado ou por voz de qualquer janela ("sim", "não"). Nada bloqueia o resto da interface: uma permissão esperando em uma conversa nunca congela outra.

## O piso duro

Comandos que tocam produção (`kubectl apply`, `terraform apply`, `helm`, force push, SQL destrutivo, exclusões na nuvem, publicação de pacotes) nunca são aprovados automaticamente, seja qual for o agente que os roda. Regras permanentes de "sempre permitir" são ignoradas para eles, o cartão fica vermelho e, no modo bypass, esses CLIs são bloqueados de vez no Claude Code. Esse bloqueio é uma flag do Claude Code sem equivalente em outro lugar, então em qualquer outro agente escolher bypass abre a conversa em accept-edits e avisa no chat. O Hark pode ser descuidado com os seus tokens; não pode ser descuidado com a sua infraestrutura.

## Modos

A pílula de permissão ao lado do compositor define a postura com que os novos turnos começam: manual (pergunta sobre tudo), aceitar edições, plano, auto, bypass. `worker_mode` na [configuração](/docs/pt-br/use/reference/configuration/) define o padrão. Nenhum modo desliga o piso.
