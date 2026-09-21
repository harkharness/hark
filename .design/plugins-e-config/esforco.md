# Plugins e configurações — o que muda e quanto custa

Pranchas em `canvas.html` (abre direto do disco, sem servidor) e nos
`*.dc.html` individuais. Geradas por `node build.mjs`.

## O que está ruim hoje

1. **A janela não cresce.** `.modal.settings` é `width: min(760px, 92vw)`
   e `max-height: min(560px, 86vh)` (`src/styles.css:1782`). Numa tela de
   1800px o painel continua com 760. O editor de `config.toml`, que entrou
   na semana passada, vive dentro dessa caixa.
2. **A tela de Plugins é sopa de pílulas.** Cada card empilha 4 pílulas de
   tier, 6 de capacidade, 1 por chave de ambiente, os argumentos e a dica
   de login — todas com o mesmo tamanho, a mesma borda e a mesma cor.
   A única decisão da tela (quem dirige as sessões) tem o mesmo peso
   visual do caminho do binário.
3. **Nada identifica um agente.** Todos os cards são retângulos de texto.
   Pior no caso dos gêmeos: `claude` e um `claude` apontado para um
   gateway mostram o mesmo nome e o mesmo `claude v2.1.236`.
4. **Capacidade ausente é invisível.** Hoje só as verdadeiras viram
   pílula, então "não reporta custo" e "não sabemos" parecem a mesma coisa.
5. **O card é um interruptor disfarçado.** Clicar troca o agente que
   dirige (`PluginsPanel.tsx:167`). Não há como inspecionar um agente
   sem passar a usá-lo.

## Decisão 1 — Ajustes vira aba da janela mãe

Vale a pena, e é mais barato do que parece:

- A engrenagem já mora na tira de abas da mãe (`Mother.tsx:1366`) e se
  comporta como aba; só o resultado é um modal. A inconsistência é gratuita.
- `mother-wide` — o layout largo que board e custos usam — já existe
  (`Mother.tsx:1337`). A aba nova herda ele sem CSS novo de layout.
- Configuração é global, como board e custos. A janela de projeto continua
  sem Ajustes; quem chamar de lá fronta a mãe, como já faz o "custos ↗".
- Esc volta para a aba anterior em vez de fechar; `cmd+,`, a voz
  ("abre as configurações") e o menu passam a trocar a aba.

**Alternativa barata, se você quiser só parar de sangrar:** trocar a
linha 1783 do `styles.css` para `width: min(1180px, 94vw); height:
min(820px, 88vh)`. Dez minutos, resolve o aperto, não resolve a
inconsistência. As duas coisas não se atrapalham — a grade de Plugins
funciona nas duas (prancha `Estreito`).

## Decisão 2 — Plugins em grade, com detalhe ao lado

- **Grade responsiva** `repeat(auto-fill, minmax(230px, 1fr))`: 2 colunas
  a 780px, 3 a 1440, 5 numa tela grande. O card carrega identidade e UMA
  linha de meta — o modelo do tier padrão e a contagem de capacidades.
- **Marca por fornecedor**, desenhada por nós: um glifo geométrico sobre o
  matiz do fornecedor. Um id desconhecido cai no monograma com matiz
  derivado do próprio id, então o gêmeo que você escreveu no `config.toml`
  também tem cara. **Nenhum SVG de marca de terceiro entra no repo.**
- **O anel da marca é o estado**: verde em uso, âmbar ausente do PATH,
  tracejado desligado. Sai uma pílula por card.
- **Painel de detalhe** ao lado (≥1040px de conteúdo) ou empilhado abaixo:
  identidade e tabela do `config.toml`, binário, argumentos, modelos por
  tier como tabela, chaves de ambiente **pelo nome** com o valor sempre
  "no arquivo", capacidades com presença E ausência marcadas, login, e o
  atalho para editar `[agents.<id>]` no editor.
- **Clique inspeciona; só "usar este" troca o motor.** É o que permite
  olhar um agente sem adotá-lo.
- **"novo agente" vira o último card** da grade e abre o detalhe em modo
  de criação, no lugar da barra de formulário solta embaixo de tudo.
- A linha do registro (checado às…, rechecar) sobe para o cabeçalho da
  seção.

## Decisão 3 — a navegação de Ajustes

Seis itens planos viram três grupos: **você** (Geral, Voz), **agentes**
(Plugins, Workers), **máquina** (Avançado, config.toml). Hoje "Workers"
está entre "Voz" e "Avançado" por acidente de ordem. As seções de
formulário ganham teto de medida de 720px; só Plugins e o editor usam a
largura inteira.

## Esforço

| # | Etapa | Tamanho | Notas |
|---|---|---|---|
| E1 | Modal cresce (uma regra de CSS) | P · 10 min | Independente; pode ir hoje |
| E2 | Ajustes vira aba da mãe | M · ~2h | `Mother.tsx` (4 pontos que abrem), `Settings.tsx` (perde o backdrop), Esc |
| E3 | Grade + marcas + painel de detalhe | G · ~1 dia | `AgentMark.tsx` novo, `PluginsPanel` se parte em grade + `AgentDetail` |
| E4 | Navegação agrupada + teto de medida | P · ~40 min | `Settings.tsx` + CSS |

Total ~1,5 dia, 3 ou 4 commits. **E3 não depende de E2**: a grade cabe no
modal de hoje (prancha `Estreito`). Ordem sugerida: E1 → E3 → E2 → E4.

### Arquivos

- `src/components/PluginsPanel.tsx` — vira a grade; perde o detalhe interno
- `src/components/AgentDetail.tsx` — novo
- `src/components/AgentMark.tsx` — novo, puro (fornecedor → glifo, id → monograma)
- `src/components/Settings.tsx` — sai do modal, navegação agrupada, medida
- `src/Mother.tsx` — `MotherTab` ganha `"ajustes"`; 4 pontos de abertura
- `src/lib/i18n.ts` — chaves novas nos dois idiomas
- `src/styles.css` — blocos `.plugin-*` reescritos; `.setview` no lugar de `.modal.settings`

### Testes (TDD, antes do código)

- `AgentMark`: fornecedor conhecido → glifo; id desconhecido → monograma
  com matiz estável entre renders; o mesmo id sempre no mesmo matiz.
- Grade: clicar num card **não** chama `agent_plugin_select`; o botão
  "usar este" chama.
- Detalhe: mostra os NOMES das chaves de ambiente e nunca um valor;
  capacidade falsa aparece marcada como ausente.
- Ajustes-aba: a engrenagem troca a aba; Esc volta para a anterior.

## O que fica de fora

- **Edição estruturada de `env` e `models` por campo.** O detalhe lê;
  quem escreve continua sendo o editor de `config.toml`. É a próxima
  fatia, não esta.
- Busca/filtro no catálogo: com 5 ou 6 agentes não paga.
- Arrastar para ordenar; ordem continua a do registro.
- A janela de projeto continua sem Ajustes.

## Riscos

- **O clique muda de significado.** Hoje clicar adota o agente; passa a
  inspecionar. É o comportamento que a tela deveria ter desde o começo,
  mas quem já usa vai estranhar uma vez.
- **O assistente de primeira execução** usa `PluginsPanel compact`
  (`Onboarding.tsx:129`) e não tem painel de detalhe. Lá o trabalho da
  tela É escolher, então em modo compacto o clique continua escolhendo —
  divergência deliberada, e o teste precisa fixá-la.
- **Matizes de fornecedor** são escolha nossa, não ativo de marca; se um
  dia isso incomodar, trocar o matiz é uma linha por fornecedor.

---

## Entregue (21/09)

Tudo acima, menos a fatia que ficou de fora de propósito (edição
estruturada de `env` e `models` por campo). E1 foi absorvido pelo E2: o
modal não cresceu, ele deixou de existir.

- **E2** — `Ajustes` é a quarta aba da mãe (`MotherTab`), com o layout
  largo que board e custos já usavam. Os quatro caminhos que abriam o
  modal (engrenagem, `cmd+,`, a voz "abre as configurações" e o pedido
  vindo de outra janela) trocam a aba; Esc volta para a aba anterior.
- **E3** — grade responsiva de cards + painel de detalhe. Marca por
  fornecedor em `AgentMark`, lógica pura em `lib/agentMark.ts`. Clique
  inspeciona, "usar este" adota. Capacidade ausente aparece marcada.
- **E4** — navegação em três grupos, cabeçalho por seção com uma linha
  de propósito, teto de medida de 720px nos formulários.

### Diferenças conscientes em relação às pranchas

1. **Sem glifo de plugue.** A prancha `Marcas` mostrava um glifo ACP
   genérico. Na implementação, fornecedor conhecido tem glifo e
   qualquer outro id cai no monograma — um plugue genérico faria dois
   agentes ACP diferentes ficarem com a mesma cara, que é o problema
   que a marca existe para resolver.
2. **O card desligado diz só "desligado".** A prancha escrevia
   "desligado · ligar", mas o clique no card passou a inspecionar, então
   o interruptor mora no detalhe. A exceção é o assistente de primeira
   execução: lá não existe detalhe, e o card mantém o botão "ligar" e o
   comando de instalação — numa máquina nova é justamente onde a
   instrução precisa estar.
3. **A contagem do card** usa as seis capacidades que o detalhe lista,
   não todos os booleanos da folha (a primeira versão dizia "11
   capacidades" para um agente com seis).

### Testes

28 testes novos (105 no front, de 77): resolução de marca e matiz
estável por id, o clique que não adota, o botão que adota, chaves de
ambiente sem valor, ausência de capacidade marcada, contagem do card,
criação de agente, as duas saídas do assistente de primeira execução,
e a tela de ajustes sem modal.
