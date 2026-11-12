---
title: Seis tipos de página
description: Documentos, apresentações, designs, planilhas, quadros brancos e dashboards, como arquivos simples em ~/.hark/artifacts.
sidebar:
  order: 1
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: db4344994c63
---

**Artefatos** é a documentação desta máquina: páginas que você e os agentes escrevem, guardadas em uma árvore como uma wiki guarda as suas, um *espaço* por projeto mais um pessoal. Uma página é um arquivo em `~/.hark/artifacts/<space>/`; a árvore é um `index.json` ao lado, e um arquivo largado ali à mão é adotado na raiz na próxima leitura. Nada aqui custa tokens para navegar.

## Os tipos

| Tipo | Arquivo | O que é |
|---|---|---|
| Documento | `.md` | markdown, renderizado como uma coluna de leitura |
| Apresentação | `.html` | uma sequência de slides em um palco de 1280×720, com o paginador, a tira de miniaturas, as setas e o modo de apresentação do Hark |
| Design | `.html` | um canvas no seu próprio tamanho em uma prancheta que o Hark aproxima e arrasta |
| Planilha | `.sheet.json` | abas de células com valores e fórmulas, calculadas pelo Hark |
| Quadro branco | `.board.json` | notas, grupos, setas e uma linha do tempo; o agente não escreve coordenadas |
| Dashboard | `.dash.json` | painéis sobre dados desta máquina: um csv, um arquivo sqlite, um comando |

Uma página html diz o que é com `<meta name="hark-kind" content="slides">`; sem a tag, uma sequência de seções `class="slide"` é lida como apresentação. HTML renderiza em um frame isolado: um documento com scripts desligados, uma apresentação ou um design com seus próprios scripts ligados; de um jeito ou de outro, o frame não tem acesso ao Hark.

## A árvore, a galeria, a página

- **A coluna da árvore** lista todos os espaços e páginas. Clique em uma página para ler, arraste uma sobre outra para movê-la para baixo dela, `+` em uma linha cria um filho, `⋯` renomeia, muda o tipo ou remove (remover apaga o arquivo e seus filhos).
- **A home de um espaço** é uma galeria: uma aba por tipo, blocos para criar uma página de cada, e um cartão por página com uma miniatura desenhada a partir do próprio arquivo.
- **Uma página** abre com seu caminho, tipo, último toque e o chat em que nasceu (o chip abre esse chat). **Links**, o painel ao lado da página, lista o chat de origem, as páginas filhas e as citações.

## Como os agentes escrevem páginas

Os agentes escrevem páginas por `HARK_ARTIFACTS_DIR`, definida em todo worker, e uma skill que o Hark instala nessa pasta diz qual espaço escolher, como nomear o arquivo e o que nunca fazer. Um `Write` na pasta é capturado como a origem da página, então a árvore sabe qual chat produziu qual página. Uma planilha, um quadro branco ou um dashboard é json que o agente escreve sem gastar um token com layout.

## Levar uma página a um chat

"Levar para…" em uma página oferece o Hark, o chat em que ela nasceu, chats recentes ou um novo, e coloca o link da página no campo desse chat; arrastar uma página para um chat na barra lateral faz o mesmo. Uma página também chega a um chat quando é citada em uma mensagem ("…como na ADR 004"), pelo seu link `hark://artifacts/<space>/<page>`, ou pelas saídas da barra de sessão.

## Compartilhar

O botão de compartilhar em uma página (e em um chat) entrega algo nesta máquina: um link local que reabre o Hark bem ali, a página como markdown ou como um único arquivo html em `~/.hark/exports`, o arquivo no Finder. Nada sai desta máquina; espaços compartilhados são uma fase posterior.
