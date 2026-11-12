---
title: Edición a mano
description: Cada página es tuya para cambiarla en el lugar; lo que el agente cambió queda marcado hasta que lo aceptas.
sidebar:
  order: 2
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: 10e8cc9f6553
---

Cada página es de la persona para editarla, y cada edición tuya es una versión que la página acepta en el acto: tus propias ediciones nunca son algo que revisar.

## Versiones y cambios marcados

Una página guarda las versiones que viste: la primera apertura toma la línea base, "aceptar" toma el texto actual, volver atrás deja la actual primero. Treinta viven ocultas junto a la página (`.history/`). Lo que un agente cambió desde la última vez que aceptaste queda marcado (celdas verdes en una hoja), con *aceptar* y *deshacer todo* arriba.

## Una hoja de cálculo

La cuadrícula propia de Hark: pestañas, una barra de fórmulas, los valores calculados aquí con un motor pequeño (`SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNTIF`, `SUMIF`, `IF`, `ROUND`, `TODAY`, y sus nombres en portugués), cero tokens. Un error que el motor no puede resolver se nombra en la celda (`#NOME?`, `#CICLO!`, `#DIV/0!`). Doble clic edita una celda y escribe el archivo. La letra de una columna abre su menú (insertar, eliminar, un formato: moneda, porcentaje, entero, fecha, texto); arrastrar la letra mueve la columna, su borde la redimensiona; un número de fila inserta o elimina filas; una pestaña se renombra con doble clic. Las fórmulas siguen a sus celdas cuando filas y columnas se mueven; una celda eliminada deja `#REF!`. La pestaña abierta se copia como csv con los valores.

## Una pizarra

Notas en un lienzo, grupos alrededor y una línea de tiempo. Arrastra una nota (dentro de un grupo la une a él), doble clic para editar, agrega notas y grupos, cambia colores. Una nota seleccionada ofrece "enlazar": el siguiente clic nombra el destino y se dibuja una flecha (haz clic en ella para etiquetarla o eliminarla). Un grupo se renombra con doble clic y su esquina lo redimensiona; la línea de tiempo abre un formulario pequeño para agregar, editar o eliminar un hito. Un lazo sobre el fondo selecciona notas, y "preguntar sobre la selección" las lleva a un chat como texto.

## Un dashboard

Paneles sobre datos de esta máquina: un archivo csv o json del proyecto, un archivo sqlite (solo lectura, solo SELECT) o un comando del proyecto que imprime csv o json. Los números se guardan en caché junto a la página con el instante en que se tomaron; "actualizar ahora" ejecuta cada fuente, y esa es la única vez que un comando corre. Una consulta que tú **certificas** es tuya: el agente la usa y nunca la reescribe. "Fuentes y consultas" muestra cada consulta con su vista previa y te deja editarla en el lugar, agregar y eliminar consultas y fuentes. Un lápiz en un panel abre su formulario (título, consulta, gráfico, columnas), con mover y eliminar; la cadencia es un selector en el encabezado. "Historia de los datos" pide al modelo ligero un párrafo sobre los paneles, que se convierte en una página hija o va a un chat después de que lo leas.

## Palabras editadas en el lugar

Con los scripts encendidos, un doble clic sobre un texto en una presentación, un diseño o un documento html lo hace editable justo ahí (Esc cancela; Enter cierra un encabezado). Hark coloca el cambio en el archivo por el texto anterior del elemento, y se niega antes que adivinar cuando ese texto no está en el archivo exactamente una vez. Un documento markdown se edita con la fuente a la izquierda y la página renderizada a la derecha.
