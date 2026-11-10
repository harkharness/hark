//! What the agents are told about pages: the skill file Hark installs and
//! the section it keeps in the soul file. Pure text; the shell writes it.
//!
//! The skill lives at `<artifacts root>/.claude/skills/hark-artifact/` and
//! the root rides as `--add-dir` on every Claude worker: the folder grants
//! writing pages and nothing else of `~/.hark`, and Claude Code loads the
//! skills of an added directory by itself. ACP agents get only the soul
//! section, which carries the same rules in short.

/// The relative place of the skill inside the artifacts root.
pub const SKILL_PATH: &str = ".claude/skills/hark-artifact/SKILL.md";

/// The soul file section that points at pages.
pub const SECTION: &str = "## Páginas (artefatos do Hark)";

/// SKILL.md: how a page is made, where it goes, what never to touch.
pub fn skill_md(root: &str) -> String {
    format!(
        r#"---
name: hark-artifact
description: Escreve ou atualiza uma página da documentação local do Hark (artefatos). Use quando o usuário pedir para documentar, registrar uma decisão, gerar um relatório, uma apresentação, um design, uma planilha, um quadro de notas ou um painel de dados que deva ficar guardado e organizado, ou disser "numa página do Hark".
---

# Páginas do Hark

O Hark guarda a documentação desta máquina em `{root}` (a variável
`HARK_ARTIFACTS_DIR` aponta para lá). Cada projeto tem um espaço com o nome
dele; páginas sem projeto vão em `pessoal`. A tela "Artefatos" do Hark lê
essa pasta e mostra tudo em árvore.

## Como criar uma página

1. Escolha o espaço: o nome do projeto em que você está, ou `pessoal`.
2. Escreva o arquivo com a ferramenta Write em
   `{root}/<espaço>/<slug>.md` (markdown), `.html` (apresentação ou
   design, um arquivo só, sem scripts externos), `.sheet.json`
   (planilha), `.board.json` (quadro) ou `.dash.json` (painel de dados).
   O slug é o título em minúsculas, sem acentos, com hífens:
   `adr-004-janela-unica.md`, `custos-de-infra.sheet.json`.
3. Para pôr a página embaixo de outra, use uma subpasta com o nome do
   arquivo da mãe: se existe `decisoes.md`, então
   `{root}/<espaço>/decisoes/adr-004.md` fica sob ela. Sem mãe, a página
   vai para a raiz e o usuário organiza depois.
4. Comece o markdown com `# Título` (ou dê um `<title>` ao html): é o
   título que a árvore mostra.

O Hark registra sozinho qual chat escreveu a página. Depois de escrever,
diga ao usuário o caminho e que ela aparece em Artefatos.

## Como fazer uma página boa

A página é lida como documento, não como resposta de chat. Capriche na
forma tanto quanto no conteúdo:

- **Documento (`.md`)**: comece com `# Título` e uma linha de abertura
  que diga em uma frase o que a página é e para quem. Depois seções
  curtas com `##`, uma ideia por parágrafo, listas só quando são listas,
  tabelas para comparar coisas (duas a quatro colunas), e no fim um
  "Ponto de partida" ou "Próximos passos" com links. Nada de despejar o
  contexto inteiro: a página explica; o detalhe fica nas fontes linkadas.
- **Apresentação ou design (`.html`)**: um arquivo só, CSS inline no
  `<style>`, fontes do sistema, sem scripts externos, sem CDN, sem
  imagens remotas; `<meta name="viewport">`, cores que funcionam em
  claro e escuro (`prefers-color-scheme`), uma ideia por tela, texto
  grande. Diga o que a página é com
  `<meta name="hark-kind" content="slides">` (ou `design`, ou `doc` para
  uma página html que é documento): é isso que dá o ícone e o visor.
  **Apresentação**: cada slide é um `<section>` direto no `<body>`,
  desenhado para 1280×720; não escreva paginador, nem teclas, nem
  contador: o Hark põe o palco, as setas, as miniaturas e o modo
  apresentar. **Design**: uma tela só, com o tamanho dito em
  `<meta name="hark-canvas" content="1440x900">`; o Hark dá o zoom e o
  arrasto. A página roda num iframe isolado, sem acesso ao Hark; um
  documento html abre com scripts desligados.
- Em qualquer formato: o título da página é o que a árvore mostra —
  escolha um que diga o assunto ("Home do workspace", "ADR 004: janela
  única"), não a ação ("atualizar artefato").

Esqueleto de apresentação, para começar:

```html
<!doctype html>
<html lang="pt-BR"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Título</title>
<meta name="hark-kind" content="slides">
<style>
  :root {{ color-scheme: light dark; --ink: #1f2430; --paper: #fff; --accent: #2f5fd1; }}
  @media (prefers-color-scheme: dark) {{ :root {{ --ink: #d6dae3; --paper: #12151c; --accent: #7aa2f7; }} }}
  body {{ margin: 0; font: 18px/1.5 -apple-system, "Segoe UI", sans-serif; color: var(--ink); background: var(--paper); }}
  section {{ min-height: 100vh; padding: 8vh 10vw; box-sizing: border-box; border-bottom: 1px solid rgba(128,128,128,.25); }}
  h1 {{ font-size: 44px; margin: 0 0 16px; letter-spacing: -.02em; }} h2 {{ font-size: 32px; margin: 0 0 12px; }}
  .kicker {{ color: var(--accent); font-size: 14px; letter-spacing: .08em; text-transform: uppercase; }}
</style></head>
<body>
<section><div class="kicker">Resumo</div><h1>Título</h1><p>Uma frase sobre o que é.</p></section>
<section><h2>Primeira ideia</h2><p>…</p></section>
</body></html>
```

## Planilha (`.sheet.json`)

Um objeto com `title` e `tabs`; cada aba tem `name`, `columns` (a linha 1,
o cabeçalho) e `rows` (as linhas de dados, da linha 2 em diante). Uma
célula é número, texto, `true`/`false`, `null`, ou uma fórmula: texto
começando com `=`. O Hark calcula as fórmulas sozinho, sem gastar
token; escreva valores como números, não como texto com ponto ou vírgula.

```json
{{ "title": "Custos de infra", "tabs": [
  {{ "name": "Mensal", "columns": ["mês", "infra", "banco", "total"],
    "rows": [
      ["set/26", 412, 96, "=B2+C2"],
      ["out/26", 438, 101, "=SOMA(B3:C3)"],
      ["média", "=MÉDIA(B2:B3)", "=MÉDIA(C2:C3)", "=SE(D3>D2; \"subiu\"; \"caiu\")"]
    ] }} ] }}
```

Fórmulas: `+ - * / ^ &`, comparações, referências `B2` e intervalos
`B2:B9` (a linha 1 é o cabeçalho), e as funções SOMA, MÉDIA, MÍN, MÁX,
CONT.NÚM, CONT.VALORES, CONT.SE, SOMA.SE, SE, E, OU, NÃO, ARRED, ABS,
HOJE, CONCATENAR, NUM.CARACT (ou os nomes em inglês). Argumentos
separados por `;` ou `,`; decimais com ponto. Uma fórmula que o Hark
não entende aparece na célula como `#NOME?`, `#CICLO!`, `#DIV/0!`,
`#VALOR!` ou `#ERRO`: corrija o arquivo. O que você mudar numa planilha
que já existia fica marcado em verde até a pessoa aceitar: mude só o
que foi pedido.

## Quadro (`.board.json`)

Notas num quadro, com grupos e uma linha do tempo. Não escreva
coordenadas: o Hark arruma as notas dentro dos grupos; a pessoa arrasta
depois. Cores: `yellow`, `blue`, `green`, `pink`, `gray`.

```json
{{ "title": "Plano da release",
  "groups": [ {{ "id": "g1", "title": "Quick-add" }}, {{ "id": "g2", "title": "Dúvidas" }} ],
  "notes": [ {{ "id": "n1", "text": "Atalho no celular", "group": "g1", "owner": "Ana", "date": "2026-11-12" }},
             {{ "id": "n2", "text": "Entra na 0.11?", "group": "g2", "color": "pink" }} ],
  "timeline": [ {{ "date": "2026-11-12", "title": "quick-add beta", "owner": "Ana" }} ] }}
```

Para agrupar por tema, dê `group` às notas e crie os grupos; para montar
uma linha do tempo do que a pessoa falou, preencha `timeline`. Ids
únicos (`n1`, `n2`…). Notas que você mudar ou criar ficam marcadas até
a pessoa aceitar; nunca apague notas que não foram pedidas.

## Painel de dados (`.dash.json`)

Painéis sobre dados que estão NESTA máquina: um arquivo do projeto
(`csv`, `json`), um banco `sqlite` (aberto só para leitura) ou um
`command` do projeto que imprime csv ou json (roda só quando a pessoa
pede para atualizar). Caminhos são relativos à raiz do projeto, nunca
sobem com `..`. Uma consulta é `sql` numa fonte sqlite (só SELECT) ou
`columns` (e `limit`) nas outras. Um painel desenha uma consulta:
`number` (com `value` e, opcional, `previous` para a variação), `bar` e
`line` (`x` rótulos, `y` valores; o Hark acha as colunas se faltar),
`table`. `refresh`: `manual`, `hourly` ou `daily`.

```json
{{ "title": "Uso do app", "refresh": "hourly",
  "sources": [ {{ "id": "db", "kind": "sqlite", "path": "data/app.db" }},
               {{ "id": "ev", "kind": "csv", "path": "data/eventos.csv" }} ],
  "queries": [ {{ "id": "por_dia", "source": "db",
                 "sql": "SELECT date(criado_em) AS dia, count(*) AS n FROM lancamentos GROUP BY dia ORDER BY dia" }},
               {{ "id": "retencao", "source": "ev", "columns": ["mes", "d7"] }} ],
  "panels":  [ {{ "title": "lançamentos por dia", "query": "por_dia", "chart": "bar", "x": "dia", "y": "n" }},
               {{ "title": "retenção D7", "query": "retencao", "chart": "line" }} ] }}
```

`"certified": true` numa consulta é marca da PESSOA: nunca escreva nem
mude uma consulta certificada; se precisar de outra coisa, crie uma
consulta nova ao lado. O que você mudar aparece como "rascunho do
agente" até ela certificar. Nunca ponha segredos nem comandos
destrutivos numa fonte.

## Ajustar, renomear

- Ajustar uma página é editar o mesmo arquivo (Edit ou Write no mesmo
  caminho). Não escreva outro arquivo com o conteúdo novo.
- Renomear é `mv` do arquivo, na mesma pasta, num passo só: a árvore
  segue, e a página guarda o chat de origem e os links. Trocar o formato
  (`home.md` → `home.html`) é escrever o novo e apagar o antigo, na mesma
  pasta: continua a mesma página. Uma página cujo arquivo some sai da
  árvore.
- Mudar de pasta (de mãe) é da pessoa, arrastando na árvore.
- Para apontar para uma página numa resposta ou em outra página, o link
  é `hark://artifacts/<espaço>/<id>`, onde `<id>` é o nome do arquivo
  sem a extensão (`custos.sheet.json` → `hark://artifacts/tulipa/custos`).
  O Hark abre o link no visor de artefatos.

## O que nunca fazer

- Nunca edite `index.json`: é o índice do Hark, ele se reconcilia sozinho.
- Nunca apague páginas que você não escreveu neste chat.
- Nada de segredos, tokens ou dados sensíveis numa página.
- Um pedido de documentar não é um pedido de refatorar: a página descreve,
  o código fica como está.
"#
    )
}

/// The soul section: short, the skill carries the rest.
pub fn persona_section(root: &str) -> String {
    format!(
        r#"{SECTION}
- A documentação local fica em `{root}` (`HARK_ARTIFACTS_DIR`): um espaço
  por projeto, páginas `.md` ou `.html`, árvore na tela "Artefatos".
- Para documentar, registrar uma decisão ou guardar um relatório, escreva
  uma página lá (skill `hark-artifact`); nunca edite `index.json`.
"#
    )
}

/// The soul file with the section appended, or None when it already has it.
/// Everything the user and the agents wrote stays byte for byte.
pub fn with_section(doc: &str, section: &str) -> Option<String> {
    if doc.contains(SECTION) {
        return None;
    }
    let mut out = doc.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(section);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skill_says_where_pages_go_and_how_they_are_named() {
        let md = skill_md("/Users/x/.hark/artifacts");
        assert!(md.starts_with("---\nname: hark-artifact\n"), "frontmatter first: {md}");
        assert!(md.contains("/Users/x/.hark/artifacts/<espaço>/"), "{md}");
        assert!(md.contains("HARK_ARTIFACTS_DIR"), "{md}");
        assert!(md.contains("index.json"), "the agent must not touch the index: {md}");
        // The three json kinds, and the sheet's own rules (10/11).
        assert!(md.contains(".sheet.json") && md.contains(".board.json") && md.contains(".dash.json"), "{md}");
        assert!(md.contains("#NOME?"), "the errors are named so the agent fixes the file: {md}");
        assert!(md.contains("\"=SOMA(B3:C3)\""), "a formula is a string starting with =: {md}");
        assert!(md.contains("\"certified\": true") && md.contains("só SELECT"), "a dashboard's rules: {md}");
        assert!(md.contains("Não escreva\ncoordenadas") && md.contains("\"timeline\""), "a board's rules: {md}");
    }

    #[test]
    fn the_skill_says_how_a_page_is_linked() {
        let skill = skill_md("/home/ana/.hark/artifacts");
        assert!(skill.contains("hark://artifacts/<espaço>/<id>"), "the link form");
        assert!(skill.contains("sem a extensão"), "the id is the file name without its extension");
    }

    #[test]
    fn the_soul_section_is_added_once_and_never_twice() {
        let doc = "# Hark\n\n## Aprendizados\n- x\n";
        let once = with_section(doc, &persona_section("/Users/x/.hark/artifacts")).expect("added");
        assert!(once.starts_with(doc), "learnings untouched: {once}");
        assert!(once.contains(SECTION), "{once}");
        assert_eq!(with_section(&once, &persona_section("/Users/x/.hark/artifacts")), None, "already there");
    }
}
