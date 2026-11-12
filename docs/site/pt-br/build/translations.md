---
title: Traduções
description: O app fala português e inglês; esta documentação fala três idiomas, e um gate as mantém em dia.
sidebar:
  order: 6
source_hash: 4d36b307c46e
---

## O app

A interface fala português e inglês (`src/lib/i18n.ts`); um idioma novo é uma tabela nova com as mesmas chaves. Títulos, cabeçalhos, itens de menu, botões, placeholders e tooltips começam com letra maiúscula; só texto que vive dentro de uma frase fica em minúsculas. As strings faladas têm sua própria tabela.

## Esta documentação

As páginas vivem no repositório em `docs/site/<lang>/`, uma árvore por idioma: `en` é a fonte, `pt-br` e `es` são traduções com os mesmos caminhos de arquivo. Uma página sem tradução cai para o inglês no site com um aviso; o gate abaixo garante que isso nunca dure.

### O gate

`node scripts/docs-check.mjs` roda a cada push e pull request, e antes de cada release. Ele falha quando:

- uma página em inglês não tem gêmea em um idioma, ou uma tradução não tem fonte em inglês;
- uma tradução foi feita a partir de uma página em inglês mais antiga: cada tradução carrega `source_hash` no seu frontmatter, o hash da página em inglês de que foi traduzida;
- o esqueleto de uma tradução se desviou: níveis de cabeçalho, número de blocos de código, número de links internos, chaves do frontmatter.

### Traduzir uma página

1. Edite ou adicione a página em inglês.
2. Escreva a mesma página em `pt-br/` e `es/`, mesmo caminho, mesmos cabeçalhos e blocos de código, links apontando para as páginas do mesmo idioma (`/docs/pt-br/…`).
3. Carimbe as traduções com o hash da página em inglês:

```bash
node scripts/docs-check.mjs --stamp docs/site/pt-br/use/start/install.md docs/site/es/use/start/install.md
```

4. Rode `node scripts/docs-check.mjs`; ele diz quando tudo está em dia.

O gate não julga a qualidade de uma tradução. Para isso existe a revisão no pull request.
