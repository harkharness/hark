---
title: Traducciones
description: La app habla portugués e inglés; esta documentación habla tres idiomas, y una compuerta las mantiene al día.
sidebar:
  order: 6
source_hash: 4d36b307c46e
---

## La app

La interfaz habla portugués e inglés (`src/lib/i18n.ts`); un idioma nuevo es una tabla nueva con las mismas claves. Títulos, encabezados, elementos de menú, botones, placeholders y tooltips empiezan con mayúscula; solo el texto que vive dentro de una frase se queda en minúscula. Las cadenas habladas tienen su propia tabla.

## Esta documentación

Las páginas viven en el repositorio bajo `docs/site/<lang>/`, un árbol por idioma: `en` es la fuente, `pt-br` y `es` son traducciones con las mismas rutas de archivo. Una página sin traducción cae al inglés en el sitio con un aviso; la compuerta de abajo se asegura de que eso nunca dure.

### La compuerta

`node scripts/docs-check.mjs` corre en cada push y pull request, y antes de cada release. Falla cuando:

- una página en inglés no tiene gemela en un idioma, o una traducción no tiene fuente en inglés;
- una traducción se hizo a partir de una página en inglés más vieja: cada traducción lleva `source_hash` en su frontmatter, el hash de la página en inglés de la que se tradujo;
- el esqueleto de una traducción se desvió: niveles de encabezado, número de bloques de código, número de enlaces internos, claves del frontmatter.

### Traducir una página

1. Edita o agrega la página en inglés.
2. Escribe la misma página en `pt-br/` y `es/`, misma ruta, mismos encabezados y bloques de código, con los enlaces apuntando a las páginas del mismo idioma (`/docs/es/…`).
3. Estampa las traducciones con el hash de la página en inglés:

```bash
node scripts/docs-check.mjs --stamp docs/site/pt-br/use/start/install.md docs/site/es/use/start/install.md
```

4. Ejecuta `node scripts/docs-check.mjs`; dice cuándo todo está al día.

La comprobación no juzga la calidad de una traducción. Para eso está la revisión en el pull request.
