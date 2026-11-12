---
title: Comentarios y @hark
description: Un comentario junto a la página, anclado a una celda, una nota, una diapositiva o una marca; @hark lo convierte en una petición.
sidebar:
  order: 3
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: 747713bce9cb
---

"Comentar" abre una columna junto a la página. Lo que esté seleccionado es el ancla (una celda, una nota, un panel, un pasaje que seleccionaste en un documento o una presentación; si no, la página entera), y el comentario se guarda junto a la página en `<page>.comments.json`, nunca dentro de ella.

## @hark

Un comentario que nombra a `@hark` es una petición. "Enviar a hark" se marca solo, el chat donde nació la página es el predeterminado (se puede elegir cualquier chat reciente, el chat de Hark o uno nuevo), y la petición va como un mensaje que nombra el enlace de la página, el ancla y tus palabras. Cuando ese chat escribe la página, el comentario dice "respondido por hark" y la página muestra la versión del agente con aceptar y deshacer, como siempre. Los comentarios simples siguen siendo tuyos y nunca entran en un chat.

## La barra de marcado

En una presentación, un diseño o un documento html, la columna de comentarios trae una barra de marcado: seleccionar (un clic sobre un texto ancla a él), lápiz, rectángulo y texto dibujan sobre la página, y la marca es el ancla, en píxeles de la página, en la diapositiva donde está, con las palabras debajo nombradas. Las marcas abiertas siguen dibujadas sobre la página, numeradas como sus comentarios.
