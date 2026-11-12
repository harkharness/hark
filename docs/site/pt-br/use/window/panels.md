---
title: Painéis ao lado de um chat
description: O terminal, a janela de arquivos, o navegador, o painel de artefato e uma janela separada.
sidebar:
  order: 2
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: f5ac098d7de0
---

Cada chat tem seus próprios painéis laterais, empilhados no trilho: a barra de sessão, o terminal, a janela de arquivos, o navegador e o painel de artefato abrem um sobre o outro; cada um dobra até sua barra, e qualquer um pode tomar o trilho inteiro. Eles pertencem ao chat em que foram abertos: volte a um chat e seu shell, seu arquivo aberto ou sua página ainda estão lá, e outro chat tem os seus.

## O chat

Transcrições em markdown de verdade: títulos, tabelas, código com realce de sintaxe, chamadas de ferramenta dobradas, rajadas de comandos de shell agrupadas. Entregas aparecem como cartões de arquivo; CSVs abrem como tabelas. Toda chamada de ferramenta privilegiada é renderizada como um [cartão de permissão](/docs/pt-br/use/window/permissions/) inline.

## O terminal

Um terminal PTY de verdade, com uma aba por shell. Um bloco de código em uma resposta oferece "rodar no terminal" e "inserir no terminal" (sem rodar).

## A janela de arquivos

A árvore do projeto à esquerda, abas para os arquivos abertos, um editor com salvar (`Cmd+S`). Um caminho em uma resposta abre ali, na linha. `Cmd+P` abre a janela com o filtro pronto.

## O navegador

Uma página de verdade ao lado do chat, em uma webview própria: sua própria origem, cookies e histórico, e nenhum caminho para alcançar o Hark. A barra é do Hark (voltar, avançar, recarregar, o endereço, abrir fora); a página é o que você apontar. Uma página por janela, lembrada por chat.

## O painel de artefato

Uma página que o agente escreveu, clicada nas saídas da barra de sessão ou pelo seu link `hark://` em uma resposta, abre ao lado do chat, nunca no lugar dele: só a página, com seus controles de fonte, comentário e "levar para". O botão de expandir do painel abre a [visão de artefatos](/docs/pt-br/use/artifacts/kinds/) completa com um botão "voltar ao chat"; cada chat lembra a página que seu painel mostrou.

## Uma janela própria

Um chat pode ser destacado em uma janela própria (o botão ↗ na barra de título). Uma janela destacada continua recebendo as tarefas do seu projeto, faladas ou clicadas, até ser fechada, e mostra o quadro filtrado para esse projeto.
