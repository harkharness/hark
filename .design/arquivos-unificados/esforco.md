# Arquivos numa janela só: visual e esforço (15/09/2026)

Canvas: os artboards `*.dc.html` desta pasta (gerados por `build.mjs`, layout em
`canvas.json`). Referência do pedido: os prints do usuário mostrando a árvore, as
abas e o visualizador num único widget, com a árvore recolhível.

## Hoje

Duas janelas tipadas na rail, cada uma com o seu `PanelFrame`:

- `arquivos` = `FilesPanel` (filtro em todos os projetos + uma árvore por projeto,
  `FileTree`). Clicar num arquivo chama `openFile`, que faz `ensureRail("arquivo")`
  e faz a SEGUNDA janela nascer.
- `arquivo` = `FilesEditor` (até 5 abas, no cabeçalho do frame) + `FileViewer`
  (markdown abre renderizado e o lápis destrava a fonte; código abre editando; CSV
  vira tabela; Cmd+S salva; bolinha âmbar = aba suja).
- Caminho clicado no chat (`code-path` do `Markdown.tsx` → `openAbsolutePath`,
  com `:linha`) e Cmd+P (`QuickOpen`) também caem em `openFile`.
- `App.tsx`: `RailItem = "arquivo" | "terminal" | "arquivos" | "board"`;
  `frameArquivo` e `frameArquivos`; `openFile` (LRU de 5, nunca despeja aba suja);
  `closeFileTab` (a última aba fecha a janela); `filesInitialProject` (clique na
  pasta da sidebar expande aquele projeto); `ensureRail` (um 3º painel expandido
  recolhe o mais velho). A rail é `useState`, não persiste: sem migração.
- `Mother.tsx`: um `PanelFrame` com `title=""` + `FileTabs` + `FilesEditor`
  (`filesOpen`), sem árvore. Arquivos que o chat da mãe nomeia chegam com
  `project: {name: "", path: ""}`.

Tamanhos: `FilesPanel` 105 linhas, `FileTree` 105, `FilesEditor` 70,
`FileViewer` 248, `PanelFrame` 75.

## Proposta (o que o canvas mostra)

1. **Uma janela `Arquivos`**: o mesmo `PanelFrame`, corpo em duas colunas. À
   esquerda a árvore (`FilesPanel` como está, 232px, largura arrastável com o
   `react-resizable-panels` que a app já usa, `autoSaveId="hark-files"`). À
   direita o visualizador (`FilesEditor`/`FileViewer` como estão). As abas seguem
   no cabeçalho.
2. **Toggle da árvore**: um `frame-ctl` com `PanelLeftClose`/`PanelLeftOpen` (lucide,
   já no bundle) à esquerda do título. Estado por janela em `localStorage`
   (`hark-files-tree`). Padrão: aberta em tela cheia e quando o frame mede
   ≥ 560px; recolhida na rail estreita. Medir o FRAME com `ResizeObserver`, não a
   janela.
3. **Estado vazio** (sem aba): ícone `FolderOpen`, "os arquivos abertos aparecem
   aqui" e uma dica que muda com a árvore (aberta: "clique num arquivo da árvore,
   ou num caminho no chat"; recolhida: "clique num caminho no chat, ou reabra a
   árvore no ícone da esquerda").
4. **Arquivo ativo marcado na árvore**: `.tree-row.on` com a mesma tinta da aba
   ativa (`--panel-2` + `--text-strong`).
5. **Filtro na coluna estreita**: placeholder curto ("filtrar arquivos…"), um ×
   pra limpar (Esc continua limpando), linha de resultado = nome do arquivo + pasta
   em `--dim` com reticências pela esquerda.
6. **Calha de números** na fonte (leitura e edição) e a linha alvo do
   `arquivo.rs:38` clicado no chat tingida com o acento. O pulo já existe
   (`OpenFile.line` → `scrollTop`); a marca é o que falta.
7. **Mãe**: a mesma janela recebendo `projects`; ganha a árvore de graça e o
   título "Arquivos" no lugar do `title=""`.
8. **Fica de fora** (estão nos prints de referência, não no chrome do Hark):
   kebab, pop-out, busca dentro do arquivo (⌘F do webview já acha) e o "+" por
   linha.

## Passos e tamanho

Front puro, zero Rust. TDD com vitest + Testing Library, no padrão de
`ModeSelect.test.tsx`.

| # | Passo | Onde | Tamanho |
|---|-------|------|---------|
| 1 | `FilesWindow.tsx`: compõe `FilesPanel` + `FilesEditor` num `PanelGroup` horizontal, toggle com estado salvo, estado vazio, medida do frame | novo, ~150 linhas | M |
| 2 | `App.tsx`: `RailItem` perde `"arquivo"`; `frameArquivo` + `frameArquivos` viram um só; `openFile` faz `ensureRail("arquivos")`; `onClose` limpa abas como o `arquivo` fazia; `filesInitialProject` abre a árvore | ~8 pontos do arquivo | M |
| 3 | `Mother.tsx`: o `PanelFrame` sem título vira `FilesWindow` com `projects` | 1 bloco | S |
| 4 | `FileViewer.tsx`: calha na `pre` de leitura e uma coluna a mais no `viewer-editwrap` (scroll sincronizado com a textarea e a `pre` de baixo); linha alvo | + CSS | S/M |
| 5 | `FilesPanel.tsx`/`FileTree.tsx`: `.tree-row.on`, × no filtro, linha de resultado, placeholder | + CSS | S |
| 6 | i18n pt/en: `files_empty_title`, `files_empty_hint_tree`, `files_empty_hint_closed`, `files_tree_toggle`; `filespanel_ph` encurta; `frame_file` sai | `i18n.ts` | S |
| 7 | Testes: estado vazio por árvore, toggle persiste, clique na árvore vira aba, fechar a última aba volta ao vazio, frame estreito nasce recolhido, mãe com árvore | `FilesWindow.test.tsx` | S |
| 8 | Docs públicas: USAGE cita uma janela "Arquivos" | harkharness/hark | XS |

Total: cerca de um dia de sessão. Dois commits, separáveis:
`feat(ui): the files and the file share one window` (passos 1 a 3 e 5 a 7) e
`feat(viewer): line numbers, and the line the chat pointed at` (passo 4).

## Decisões pendentes e riscos

- Fechar a janela com aba suja hoje descarta o rascunho (`setOpenFiles([])`).
  Continua igual neste corte; confirmar antes é um passo à parte (S).
- `PanelGroup` aninhado dentro de um `Panel` da rail: suportado no
  `react-resizable-panels` 2.x. O handle é o mesmo `.rhandle` hairline.
- Na rail, a árvore aberta + visualizador cabem a partir de ~560px de frame;
  abaixo disso o padrão recolhido evita duas colunas espremidas. O usuário pode
  abrir mesmo assim; o estado salvo vence o padrão.
- LRU de 5 abas, `expanded` (tela cheia) e o cabeçalho do frame não mudam.

## Entregue (15/09/2026, mesma sessão)

Implementado como um commit só (a calha não valia o malabarismo de separar
hunks do `styles.css`): `FilesWindow.tsx` + `TreeToggle`, `useFilesTree` +
`lib/filesTree.ts` (puro, testado), `PanelFrame.lead`, árvore com revelar e
marcar, filtro com ×, setas, Enter e linha nome + pasta, calha e linha alvo
no `FileViewer`, App e mãe fiados, i18n pt/en. `RailItem` perdeu `"arquivo"`;
fechar a última aba deixa o estado vazio em vez de matar a janela; a pasta
da sidebar abre a árvore mesmo na rail. Testes: 69 (54 + 15 novos), `tsc` e
`vite build` limpos. Verificação visual pelo harness local em
`harness/files.html` (servido pelo `vite` da app com o backend falso na
fronteira do Tauri), sem artefato online.

Fora deste corte, como previsto: confirmar antes de fechar com aba suja,
busca dentro do arquivo, kebab, pop-out.

## Verificação

- `npx vitest run` (54 hoje + os novos), `npx tsc --noEmit`, `npm run build`.
- Manual: pasta na sidebar → janela com árvore e estado vazio; caminho no chat
  com `:linha` → aba aberta e linha marcada; filtro → abre à direita;
  recolher/abrir persiste ao fechar e reabrir a janela; rail estreita nasce
  recolhida; tela cheia abre a árvore; a mãe abre com árvore; Cmd+S continua
  salvando com a bolinha âmbar na aba.
