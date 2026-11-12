---
title: Frontend e gramática visual
description: React 18 + TypeScript + Vite; um orquestrador, componentes puros, um modo demo para capturas de tela.
sidebar:
  order: 4
source_hash: 1c0fefe7b1fe
---

React 18 + TypeScript + Vite, só componentes funcionais. O layout espelha a filosofia do lado Rust: renderização pura nas bordas, estado e efeitos colaterais concentrados em um orquestrador (`src/App.tsx`). `src/lib/ipc.ts` é o único lugar em que um comando Tauri é invocado, `src/lib/events.ts` o único lugar em que eventos são escutados, `src/lib/*.ts` as regras puras (escopo, busca, diff, saídas, artefatos), `src/components/` a apresentação com o estado chegando por props.

## Regras

1. **Componentes nunca chamam `invoke`.** Tudo passa por `lib/ipc.ts`, então nomes de comando e payloads têm uma casa só.
2. **O estado vive no App.** Componentes recebem dados e callbacks; estado transitório de UI (um menu aberto, um rascunho) pode ficar local, qualquer coisa que outro componente lê vive no App.
3. **Conversas são indexadas pelo rótulo da tarefa**, não pelo id, porque a transcrição sobrevive a reinícios do worker. Uma linha relida do arquivo de sessão é descartada quando a conversa já a mostra.
4. **Eventos do backend chegam só por `useHarkEvents`.** Um tipo novo de evento ganha um ramo ali e uma variante em `types.ts`.
5. **Funcionalidades de zero tokens** (visualizador de arquivos, abertura rápida, ações do quadro) nunca iniciam um processo de agente. Se uma funcionalidade custa dinheiro, ela passa pelos fluxos de gate e roteador no App.
6. **A janela roda sob uma CSP.** Scripts só do bundle; o HTML de artefatos renderiza em um `<iframe sandbox>` que não adiciona origem.

## Modo demo

`npm run dev`, depois `http://localhost:1420/?demo=<scene>` em qualquer navegador: todo comando Tauri é respondido por `lib/demo.ts` e todo evento vem do script da cena, então a janela roda sem backend. As cenas são `home`, `inicio`, `task`, `board`, `docs` e `costs`. As capturas de tela do README e do site são tiradas aqui, em 1440×900 e 2×; nada nelas é um projeto, sessão ou nome real. `demo.test.ts` falha quando um comando que a costura conhece não tem resposta no demo.

## Adicionar um comando Tauri

1. Implemente em `src-tauri/src/lib.rs` e registre em `invoke_handler`.
2. Adicione um wrapper tipado em `src/lib/ipc.ts`.
3. Adicione ou estenda os tipos de payload em `src/types.ts` (nomes de campo continuam em snake_case, como serializados).

## A gramática visual

`docs/CHROME.md` no repositório é a gramática visual: meça, não olhe; tokens em `:root` (uma escala tipográfica em seis tamanhos, corpo em 14px; cores; bordas finas); separe pelo que o conteúdo faz, não desenhando um limite; divisores são linhas finas com área de pega; uma coluna, um recuo; toda janela tem a barra de título sobreposta; duas portas, uma barra lateral; cabeçalhos de painel; e as armadilhas de cascata que fizeram várias dessas coisas parecerem decisões de design quando não eram. Títulos, cabeçalhos, itens de menu, botões, placeholders e tooltips começam com letra maiúscula.
