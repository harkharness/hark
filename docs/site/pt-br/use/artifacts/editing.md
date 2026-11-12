---
title: Edição à mão
description: Toda página é sua para mudar no lugar; o que o agente mudou fica marcado até você aceitar.
sidebar:
  order: 2
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 10e8cc9f6553
---

Toda página é da pessoa para editar, e cada edição sua é uma versão que a página aceita na hora: suas próprias edições nunca são algo a revisar.

## Versões e mudanças marcadas

Uma página guarda as versões que você viu: a primeira abertura toma a base, "aceitar" toma o texto atual, voltar mantém o atual em primeiro. Trinta vivem escondidas ao lado da página (`.history/`). O que um agente mudou desde que você aceitou pela última vez fica marcado (células verdes em uma planilha), com *aceitar* e *desfazer tudo* acima.

## Uma planilha

A grade do próprio Hark: abas, uma barra de fórmulas, os valores calculados aqui com um motor pequeno (`SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNTIF`, `SUMIF`, `IF`, `ROUND`, `TODAY`, e seus nomes em português), zero tokens. Um erro que o motor não resolve é nomeado na célula (`#NOME?`, `#CICLO!`, `#DIV/0!`). Duplo clique edita uma célula e escreve o arquivo. A letra de uma coluna abre seu menu (inserir, remover, um formato: dinheiro, porcentagem, inteiro, data, texto); arrastar a letra move a coluna, sua borda redimensiona; o número de uma linha insere ou remove linhas; uma aba renomeia com duplo clique. As fórmulas seguem suas células quando linhas e colunas se movem; uma célula removida deixa `#REF!`. A aba aberta copia como csv com valores.

## Um quadro branco

Notas em um canvas, grupos ao redor e uma linha do tempo. Arraste uma nota (para dentro de um grupo a inclui), duplo clique para editar, adicione notas e grupos, mude cores. Uma nota selecionada oferece "ligar": o próximo clique nomeia o alvo e uma seta é desenhada (clique nela para rotular ou remover). Um grupo renomeia com duplo clique e seu canto redimensiona; a linha do tempo abre um formulário pequeno para adicionar, editar ou remover um marco. Um laço no fundo seleciona notas, e "perguntar sobre a seleção" as leva a um chat como texto.

## Um dashboard

Painéis sobre dados desta máquina: um arquivo csv ou json do projeto, um arquivo sqlite (só leitura, só SELECT) ou um comando do projeto que imprime csv ou json. Os números ficam em cache ao lado da página com o instante em que foram tomados; "atualizar agora" roda todas as fontes, e essa é a única hora em que um comando roda. Uma consulta que você **certifica** é sua: o agente a usa e nunca a reescreve. "Fontes e consultas" mostra cada consulta com sua prévia e deixa você editá-la no lugar, adicionar e remover consultas e fontes. Um lápis em um painel abre seu formulário (título, consulta, gráfico, colunas), com mover e remover; a cadência é um seletor no cabeçalho. "História dos dados" pede ao modelo leve um parágrafo sobre os painéis, que vira uma página filha ou vai para um chat depois de você ler.

## Palavras editadas no lugar

Com scripts ligados, um duplo clique em um texto de uma apresentação, um design ou um documento html o torna editável bem ali (Esc cancela; Enter fecha um título). O Hark coloca a mudança no arquivo pelo texto antigo do elemento, e se recusa em vez de adivinhar quando esse texto não está no arquivo exatamente uma vez. Um documento markdown edita com a fonte à esquerda e a página renderizada à direita.
