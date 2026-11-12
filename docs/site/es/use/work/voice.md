---
title: Voz
description: Un atajo global, lo que oyó y adónde va antes de que algo corra, respuestas en voz alta.
sidebar:
  order: 3
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: 221a58943ab9
---

Presiona el atajo (`cmd+shift+space` por defecto) en cualquier lugar, en cualquier app. Con la ventana principal al frente, el propio compositor escucha: la ronda se dibuja sobre el texto (forma de onda, lo que oyó, el chip del destino) y la ventana se queda donde estabas leyendo. En cualquier otro lugar se abre en su lugar un HUD flotante. De cualquier forma se detiene sola tras un instante de silencio, y el botón del micrófono en el compositor inicia la misma ronda.

## Lo que oyó, adónde va

Antes de que algo corra, Hark te muestra dos cosas: lo que oyó y adónde va. La confirmación llega a una de dos velocidades:

- **El destino es el chat en el que ya estás**: aparece un chip y 1.6 segundos de silencio lo ejecutan.
- **Hark tuvo que resolver el destino**: dice el destino en voz alta y espera un veredicto. Di "sí" para seguir, "no" para cancelar, o di una instrucción distinta y reemplaza el texto y vuelve a preguntar. La ambigüedad nunca se resuelve adivinando: recibes candidatos numerados y eliges uno ("el primero").

Las respuestas se dicen de vuelta con la voz del sistema mientras los detalles se muestran en pantalla. El trabajo enviado por voz nunca cambia lo que estás mirando: el chat donde cayó pulsa en la barra lateral por un instante.

## Cuatro verbos hablados

"Enfócate en billing" fija Início a ese chat; "volver" o "pausa" lo libera. En un foco, una frase sin verbo va al chat en foco. "Piensa más" y "una rápida" enrutan una pregunta hacia arriba o hacia abajo.

## Cuándo Hark habla por su cuenta

Nunca con la ventana al frente: el texto hace ese trabajo. Detrás de la ventana, un chat que termina su turno se dice en voz alta, salvo que estés en un foco; entonces solo se dice el final del chat en foco, y todo lo demás espera detrás de la campana.

## Vocabulario

`vocab` en la [configuración](/docs/es/use/reference/configuration/) sesga la transcripción hacia tus términos (nombres de proyectos, "pull request", "deploy"). La gramática hablada acepta portugués e inglés al mismo tiempo.
