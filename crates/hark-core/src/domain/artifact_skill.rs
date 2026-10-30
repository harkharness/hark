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
description: Escreve ou atualiza uma página da documentação local do Hark (artefatos). Use quando o usuário pedir para documentar, registrar uma decisão, gerar um relatório, uma apresentação ou um design que deva ficar guardado e organizado, ou disser "numa página do Hark".
---

# Páginas do Hark

O Hark guarda a documentação desta máquina em `{root}` (a variável
`HARK_ARTIFACTS_DIR` aponta para lá). Cada projeto tem um espaço com o nome
dele; páginas sem projeto vão em `pessoal`. A tela "Artefatos" do Hark lê
essa pasta e mostra tudo em árvore.

## Como criar uma página

1. Escolha o espaço: o nome do projeto em que você está, ou `pessoal`.
2. Escreva o arquivo com a ferramenta Write em
   `{root}/<espaço>/<slug>.md` (markdown) ou `.html` (apresentação ou
   design, um arquivo só, sem scripts externos). O slug é o título em
   minúsculas, sem acentos, com hífens: `adr-004-janela-unica.md`.
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
  grande. A página roda isolada no Hark (iframe sem scripts por padrão),
  então o que não for HTML+CSS puro não aparece.
- Em qualquer formato: o título da página é o que a árvore mostra —
  escolha um que diga o assunto ("Home do workspace", "ADR 004: janela
  única"), não a ação ("atualizar artefato").

Esqueleto de apresentação, para começar:

```html
<!doctype html>
<html lang="pt-BR"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Título</title>
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
