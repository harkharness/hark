---
title: Comentários e @hark
description: Um comentário ao lado da página, ancorado a uma célula, uma nota, um slide ou uma marcação; @hark o transforma em pedido.
sidebar:
  order: 3
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 747713bce9cb
---

"Comentar" abre uma coluna ao lado da página. O que estiver selecionado é a âncora (uma célula, uma nota, um painel, um trecho que você selecionou em um documento ou em uma apresentação; senão, a página inteira), e o comentário fica ao lado da página em `<page>.comments.json`, nunca dentro dela.

## @hark

Um comentário que cita `@hark` é um pedido. "Enviar ao hark" se marca sozinho, o chat em que a página nasceu é o padrão (qualquer chat recente, o chat do Hark ou um novo podem ser escolhidos), e o pedido vai como uma mensagem citando o link da página, a âncora e suas palavras. Quando esse chat escreve a página, o comentário passa a dizer "respondido pelo hark" e a página mostra a versão do agente com aceitar e desfazer, como sempre. Comentários simples continuam seus e nunca entram em um chat.

## A barra de marcação

Em uma apresentação, um design ou um documento html, a coluna de comentários traz uma barra de marcação: selecionar (um clique em um texto ancora nele), caneta, retângulo e texto desenham sobre a página, e a marcação é a âncora, em pixels da página, no slide em que está, com as palavras sob ela nomeadas. Marcações abertas continuam desenhadas sobre a página, numeradas como seus comentários.
