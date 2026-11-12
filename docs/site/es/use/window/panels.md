---
title: Paneles junto a un chat
description: La terminal, la ventana de archivos, el navegador, el panel de artefactos y una ventana propia.
sidebar:
  order: 2
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: f5ac098d7de0
---

Cada chat guarda sus propios paneles laterales, apilados en el riel: la barra de sesión, la terminal, la ventana de archivos, el navegador y el panel de artefactos se abren uno sobre otro; cada uno se pliega a su barra, y cualquiera puede ocupar todo el riel. Pertenecen al chat donde se abrieron: vuelve a un chat y su shell, su archivo abierto o su página siguen allí, y otro chat tiene los suyos.

## El chat

Transcripciones en markdown real: encabezados, tablas, código con resaltado de sintaxis, llamadas a herramientas plegadas, ráfagas de comandos de shell agrupadas. Los entregables aparecen como tarjetas de archivo; los CSV se abren como tablas. Cada llamada a una herramienta privilegiada se muestra como una [tarjeta de permiso](/docs/es/use/window/permissions/) en línea.

## La terminal

Una terminal PTY real, con una pestaña por shell. Un bloque de código en una respuesta ofrece "ejecutar en la terminal" e "insertar en la terminal" (sin ejecutar).

## La ventana de archivos

El árbol del proyecto a la izquierda, pestañas para los archivos abiertos, un editor con guardado (`Cmd+S`). Una ruta en una respuesta se abre allí, en su línea. `Cmd+P` la abre con el filtro listo.

## El navegador

Una página real junto al chat, en un webview propio: su propio origen, cookies e historial, y sin forma de alcanzar a Hark. La barra es de Hark (atrás, adelante, recargar, la dirección, abrir afuera); la página es la que tú indiques. Una página por ventana, recordada por chat.

## El panel de artefactos

Una página que el agente escribió, al hacer clic en las salidas de la barra de sesión o en su enlace `hark://` dentro de una respuesta, se abre junto al chat, nunca en su lugar: la página sola, con sus controles de fuente, comentario y "llevar a". El botón de expandir del panel abre la [vista de artefactos](/docs/es/use/artifacts/kinds/) completa con un botón "volver al chat"; cada chat recuerda la página que su panel mostraba.

## Una ventana propia

Un chat se puede separar en una ventana propia (el botón ↗ en la barra de título). Una ventana separada sigue recibiendo las tareas de su proyecto, dictadas o por clic, hasta que se cierra, y muestra el tablero filtrado a ese proyecto.
