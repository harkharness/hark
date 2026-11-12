---
title: Custos e limites
description: Um livro-razão local de cada turno, honesto sobre o que não sabe.
sidebar:
  order: 2
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: e2074efec1da
---

Cada turno cai em um livro-razão SQLite local: tipo, projeto, tarefa, modelo, tokens, custo, se falhou. Configurações › Custos detalha por dia ou semana, por tipo, projeto, tarefa e modelo, mostra sua taxa de acerto de cache, o que a arquitetura evitou gastar, e lista as sessões mais caras.

## Duas regras mantêm os números honestos

- **USD vem só do relato do próprio agente.** Não existe tabela de preços em lugar nenhum do Hark. Claude Code e o adaptador ACP do Claude precificam seus turnos; Codex relata só tokens; Gemini não relata nada. Um turno que ninguém precificou aparece como `$ –`, nunca como `$0.00`.
- **Contagens de tokens e dólares nunca são somadas juntas.** Vêm de fontes diferentes, o stream ao vivo e os logs de sessão, e os logs cobrem todo o seu uso do agente, não só o que passou pelo Hark.

## O teto

Cada turno roda sob um teto em dólares (`worker_budget_usd`, padrão 5) mantido pelo próprio Hark: ele precifica as chamadas do turno conforme o stream relata e corta o turno entre chamadas ao passar do teto, então diz quanto, e oferece subir o teto para esse chat, reiniciar leve ou trocar de modelo. Em todo agente.

## O caminho barato é o padrão

Perguntas curtas e inteiras sobre os workers rodando, o quadro, o gasto e a lista do dia são respondidas pelo índice local por zero tokens. Perguntas só de formulação vão a um modelo leve com contexto pequeno. Só raciocínio de verdade paga o preço cheio. Diga "pensa mais" para subir ou "rapidinho" para descer, por pergunta.

## Janelas da assinatura

As porcentagens das janelas de 5h e 7d vêm de uma ponte opcional em torno da linha de status do Claude Code (Configurações › Custos), ou do adaptador ACP do Claude, que as relata a cada turno.
