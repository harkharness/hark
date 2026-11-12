---
title: Seis tipos de página
description: Documentos, presentaciones, diseños, hojas de cálculo, pizarras y dashboards, como archivos planos bajo ~/.hark/artifacts.
sidebar:
  order: 1
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: db4344994c63
---

**Artefactos** es la documentación de esta máquina: páginas que tú y los agentes escriben, guardadas como un árbol, como una wiki las guarda, un *espacio* por proyecto más uno personal. Una página es un archivo bajo `~/.hark/artifacts/<space>/`; el árbol es un `index.json` a su lado, y un archivo dejado allí a mano se adopta en la raíz en la próxima lectura. Navegar aquí no cuesta tokens.

## Los tipos

| Tipo | Archivo | Qué es |
|---|---|---|
| Documento | `.md` | markdown, mostrado como una columna de lectura |
| Presentación | `.html` | una serie de diapositivas en un escenario de 1280×720, con el paginador, la tira de miniaturas, las flechas del teclado y el modo de presentación de Hark |
| Diseño | `.html` | un lienzo a su propio tamaño sobre un tablero que Hark acerca y desplaza |
| Hoja de cálculo | `.sheet.json` | pestañas de celdas con valores y fórmulas, calculadas por Hark |
| Pizarra | `.board.json` | notas, grupos, flechas y una línea de tiempo; el agente no escribe coordenadas |
| Dashboard | `.dash.json` | paneles sobre datos de esta máquina: un csv, un archivo sqlite, un comando |

Una página html dice lo que es con `<meta name="hark-kind" content="slides">`; sin la etiqueta, una serie de secciones `class="slide"` se lee como presentación. El HTML se muestra en un marco aislado: un documento con los scripts apagados, una presentación o un diseño con sus propios scripts encendidos; en ambos casos el marco no tiene acceso a Hark.

## El árbol, la galería, la página

- **La columna del árbol** lista cada espacio y cada página. Haz clic en una página para leerla, arrastra una sobre otra para moverla debajo, `+` en una fila crea una hija, `⋯` renombra, cambia el tipo o elimina (eliminar borra el archivo y sus hijas).
- **El inicio de un espacio** es una galería: una pestaña por tipo, mosaicos para crear una página de cada uno, y una tarjeta por página con una miniatura dibujada a partir del propio archivo.
- **Una página** se abre con su ruta de navegación, el tipo, el último toque y el chat donde nació (el chip abre ese chat). **Enlaces**, el panel junto a la página, lista el chat de origen, las páginas hijas y las citas.

## Cómo los agentes escriben páginas

Los agentes escriben páginas a través de `HARK_ARTIFACTS_DIR`, definida en cada worker, y una skill que Hark instala bajo esa carpeta dice qué espacio elegir, cómo nombrar el archivo y qué no hacer nunca. Un `Write` en la carpeta se captura como el origen de la página, así que el árbol sabe qué chat produjo qué página. Una hoja de cálculo, una pizarra o un dashboard son json que el agente puede escribir sin gastar un token en el diseño.

## Llevar una página a un chat

"Llevar a…" en una página ofrece Hark, el chat donde nació, los chats recientes o uno nuevo, y pone el enlace de la página en el campo de ese chat; arrastrar una página sobre un chat en la barra lateral hace lo mismo. Una página también llega a un chat al nombrarla en un mensaje ("…como en el ADR 004"), por su enlace `hark://artifacts/<space>/<page>`, o a través de las salidas de la barra de sesión.

## Compartir

El botón de compartir en una página (y en un chat) entrega algo en esta máquina: un enlace local que reabre Hark justo ahí, la página como markdown o como un solo archivo html en `~/.hark/exports`, el archivo en el Finder. Nada sale de esta máquina; los espacios compartidos son una fase posterior.
