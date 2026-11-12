---
title: Voz
description: Um atalho global, o que foi ouvido e para onde vai antes de qualquer coisa rodar, respostas em voz alta.
sidebar:
  order: 3
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 221a58943ab9
---

Pressione o atalho (`cmd+shift+space` por padrão) em qualquer lugar, em qualquer app. Com a janela principal na frente, o próprio compositor escuta: a rodada se desenha sobre o texto (forma de onda, o que foi ouvido, o chip do alvo) e a janela fica onde você estava lendo. Em qualquer outro lugar, um HUD flutuante abre no lugar. De um jeito ou de outro, ele para sozinho depois de um instante de silêncio, e o botão de microfone no compositor inicia a mesma rodada.

## O que foi ouvido, para onde vai

Antes de qualquer coisa rodar, o Hark mostra duas coisas: o que ouviu e para onde vai. A confirmação vem em uma de duas velocidades:

- **O alvo é o chat em que você já está**: um chip aparece e 1,6 segundo de silêncio executa.
- **O Hark precisou resolver o alvo**: ele diz o destino em voz alta e espera um veredito. Diga "sim" para ir, "não" para cancelar, ou diga outra instrução e ela substitui o texto e pergunta de novo. Ambiguidade nunca é resolvida no chute: você recebe candidatos numerados e escolhe um ("o primeiro").

As respostas são faladas pela voz do sistema enquanto os detalhes aparecem na tela. Trabalho enviado por voz nunca troca o que você está olhando: o chat em que caiu pulsa na barra lateral por um instante.

## Quatro verbos falados

"Foca em billing" prende o Início a esse chat; "volta" ou "pausa" solta. Em um foco, uma frase sem verbo vai para o chat em foco. "Pensa mais" e "rapidinho" roteiam uma pergunta para cima ou para baixo.

## Quando o Hark fala sozinho

Nunca com a janela na frente: o texto faz esse trabalho. Atrás da janela, um chat terminando seu turno é dito em voz alta, a menos que você esteja em um foco; aí só o chat em foco terminando é dito, e todo o resto espera atrás do sino.

## Vocabulário

`vocab` na [configuração](/docs/pt-br/use/reference/configuration/) inclina a transcrição para os seus termos (nomes de projeto, "pull request", "deploy"). A gramática falada aceita português e inglês ao mesmo tempo.
