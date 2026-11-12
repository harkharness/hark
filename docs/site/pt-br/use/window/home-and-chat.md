---
title: Início e o chat
description: Início é o chat do próprio Hark sobre todos os projetos; Code é onde o trabalho vive.
sidebar:
  order: 1
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 7eb1c02ebb6f
---

Uma janela reúne todos os projetos, atrás de duas portas no topo da barra lateral, como no app do Claude Code. **Início** é o chat do próprio Hark, uma conversa sobre todos os projetos (*Hoje*), com a lista do dia ao lado. **Code** é onde o trabalho vive: **Criar** abre um chat novo (o projeto vem primeiro; uma pasta ainda não registrada vira um projeto), depois **Procurar**, os artefatos, o atalho do quadro e os projetos com seus chats, do mais novo para o mais antigo, a partir do índice local. Cada projeto tem uma cor; ela desce pelo grupo da barra lateral, atravessa o topo do chat e chega ao chip de escopo do compositor, que diz onde a próxima mensagem vai cair.

## Hoje é o próprio Hark

Início › Hoje não é uma sessão de agente: um chat sobre todos os projetos, guardado em um diário local, com os dias anteriores listados na barra lateral. Perguntas que ele consegue responder pelo índice local (o que está no quadro, quais workers estão rodando, o que você gastou hoje) nunca chegam a um modelo. Perguntas que só precisam de reformulação vão para um modelo leve com contexto pequeno. Só raciocínio de verdade paga o preço cheio. Uma frase que é trabalho ("continua a migração do webhook") é resolvida contra todos os projetos e confirmada antes de rodar; trabalho sem um chat para cair recebe uma dica, não uma sessão.

Sem nenhum chat ainda, o Início recebe você com uma linha de contagens, quatro entradas (retomar o mais recente, falar um trabalho, achar um chat, abrir o quadro) e o compositor. Quando o dia já tem um chat, Hoje é esse chat.

## Demandas: a lista do dia

O que pede por você, de cinco fontes: os chats das últimas 24 horas, um cartão do quadro parado em uma permissão ou esperando você, um parado há dias, uma página que um agente escreveu no último dia e seus próprios pedidos ("me lembra de ligar pro banco às 14h", digitados ou falados). A lista é agrupada pelo que pede (precisa de você, para revisar, lembretes, agendados, em andamento, parados); *ir* abre o chat ou a página, *depois* afunda uma linha pelo dia, *feito* fecha um pedido. "O que eu tenho hoje?" no Hoje responde a partir dessa lista, zero tokens. `hark demands` imprime a mesma lista no terminal.

## Foco

"Foca em billing" (digitado no Hoje, ou *foco* em uma demanda) prende o compositor do Início a esse chat sem sair do Início: um chip no topo diz qual, o que você escreve vai para lá, e um espelho condensado mostra suas ferramentas em uma linha cada. "Volta" ou "pausa" solta.

## O sino

O Hark nunca interrompe um turno em que você está. O que termina, pergunta ou fica pronto enquanto você está em outro lugar vira uma linha atrás do sino no topo: o turno de um chat terminou, uma permissão foi pedida, uma página que um agente escreveu. A barra lateral mostra o chat em negrito com um ponto vermelho; abrir o chat lê suas linhas.

## Duas teclas

`Cmd+P` abre a janela de arquivos com o filtro pronto. `Cmd+K` abre **Procurar**, uma paleta sobre tudo o que a janela conhece (chats, páginas, projetos, o histórico do Claude Code, algumas ações) com uma aba por tipo. Índice local, zero tokens.

## Um chat novo ganha nome

A primeira mensagem de um chat, quando se lê como uma frase, é entregue ao modelo leve para um título curto (alguns centavos, uma vez). Renomear é do chat: "renomeia esse chat pra dns fix", `/rename`, ou o menu da linha. O nome é escrito no próprio histórico da sessão, então o CLI também o mostra.
