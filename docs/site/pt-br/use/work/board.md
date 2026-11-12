---
title: O quadro e o despacho de trabalho
description: Um kanban em que cada cartão é uma sessão; trabalho resolvido contra as sessões no seu disco.
sidebar:
  order: 1
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 0ec92808eb43
---

## Despachar trabalho

Fale, ou digite, o que você quer feito: *"continua a migração do webhook"*. O Hark resolve isso contra as sessões no seu disco por palavras inteiras (nunca substrings, então "core" não casa com "hark-core"), retoma a vencedora no seu próprio workspace com suas configurações completas e transmite cada evento de volta para a janela. Um termo raro pesa mais que um comum; um empate aparece como candidatos em vez de um chute.

Antes de um worker caro começar, um avaliador barato confere a instrução contra a sessão em que ela está prestes a cair: alguns centavos para evitar dólares gastos na conversa errada. Sessões pesadas recebem um aviso honesto antes ("histórico de 3,8 MB") com a opção de compactar antes de enviar. Workers rodam em paralelo entre projetos, e cada turno, inclusive os que falham, cai no livro-razão.

## O quadro

Um kanban de tarefas, no Início ao lado de Hoje e Demandas (a porta Code guarda um atalho que o abre como painel ao lado do chat). Uma **tarefa é um registro deliberado**: nada cria uma por você. Ela nasce quando você pede ("cria tarefa corrigir o dns"), quando você aceita a oferta do próprio chat (depois de alguns turnos em um chat sem cartão, uma linha acima do compositor oferece registrá-lo, uma vez), ou no próprio quadro. Uma tarefa fica ligada ao chat em que foi pedida; um chat pode carregar várias tarefas, e uma tarefa pode reunir vários chats. Mover um cartão acompanha as conversas que fazem o trabalho, então retomar significa retomar, não começar de novo. Um plano aprovado vira a lista de verificação do cartão; trabalho terminado afunda para concluído sozinho.

## Janelas de projeto

O quadro é global; uma janela de projeto destacada mostra sua fatia, mais o que o projeto gastou.
