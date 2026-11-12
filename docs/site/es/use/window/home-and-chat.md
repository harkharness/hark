---
title: Início y el chat
description: Início es el chat propio de Hark sobre todos los proyectos; Code es donde vive el trabajo.
sidebar:
  order: 1
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: 7eb1c02ebb6f
---

Una ventana reúne todos los proyectos, detrás de dos puertas en la parte superior de la barra lateral, como en la app de Claude Code. **Início** es el chat propio de Hark, una sola conversación sobre todos los proyectos (*Hoje*), con la lista del día al lado. **Code** es donde vive el trabajo: **Criar** abre un chat nuevo (primero el proyecto; una carpeta aún no registrada se convierte en proyecto), luego **Procurar**, los artefactos, el acceso directo al tablero y los proyectos con sus chats, los más recientes primero, desde el índice local. Cada proyecto tiene un color; recorre su grupo en la barra lateral, la parte superior del chat y el chip de alcance del compositor, que dice dónde cae el próximo mensaje.

## Hoje es Hark mismo

Início › Hoje no es una sesión de agente: un solo chat sobre todos los proyectos, guardado en un diario local, con los días anteriores listados en la barra lateral. Las preguntas que puede responder desde el índice local (qué hay en el tablero, qué workers corren, cuánto gastaste hoy) nunca llegan a un modelo. Las que solo necesitan reformular van a un modelo ligero con un contexto pequeño. Solo el razonamiento real paga precio completo. Una frase que es trabajo ("continúa la migración del webhook") se resuelve contra todos los proyectos y se confirma antes de correr; un trabajo sin chat donde caer recibe una pista, no una sesión.

Sin ningún chat todavía, Início te recibe con una línea de conteos, cuatro entradas (retomar el último, dictar un trabajo, buscar un chat, abrir el tablero) y el compositor. Cuando el día ya tiene un chat, Hoje es ese chat.

## Demandas: la lista del día

Lo que te pide atención, desde cinco fuentes: los chats de las últimas 24 horas, una tarjeta del tablero detenida en un permiso o esperándote, una estancada hace días, una página que un agente escribió en el último día y tus propias peticiones ("recuérdame llamar al banco a las 2pm", escritas o dictadas). La lista se agrupa por lo que pide (te necesitan, para revisar, recordatorios, programadas, en curso, estancadas); *ir* abre el chat o la página, *después* hunde una fila por el día, *listo* cierra una petición. "¿Qué tengo hoy?" en Hoje responde desde esta lista, cero tokens. `hark demands` imprime la misma lista en la terminal.

## Foco

"Enfócate en billing" (escrito en Hoje, o *enfocar* en una demanda) fija el compositor de Início a ese chat sin salir de Início: un chip arriba dice cuál, lo que escribes va allí y un espejo condensado muestra sus herramientas en una línea cada una. "Volver" o "pausa" lo libera.

## La campana

Hark nunca interrumpe un turno en el que estás. Lo que termina, pregunta o queda listo mientras estás en otro lugar se convierte en una fila detrás de la campana de arriba: el turno de un chat terminó, un permiso pedido, una página que un agente escribió. La barra lateral muestra el chat en negrita con un punto rojo; abrir el chat marca sus filas como leídas.

## Dos teclas

`Cmd+P` abre la ventana de archivos con el filtro listo. `Cmd+K` abre **Procurar**, una paleta sobre todo lo que la ventana conoce (chats, páginas, proyectos, el historial de Claude Code, algunas acciones) con una pestaña por tipo. Índice local, cero tokens.

## Un chat nuevo recibe un nombre

El primer mensaje de un chat, cuando se lee como una frase, se entrega al modelo ligero para un título corto (unos centavos, una vez). Renombrar es del chat: "renombra este chat a dns fix", `/rename`, o el menú de la fila. El nombre se escribe en el historial propio de la sesión, así que el CLI también lo muestra.
