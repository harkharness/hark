---
title: El tablero y el despacho de trabajo
description: Un kanban donde cada tarjeta es una sesión; trabajo resuelto contra las sesiones en tu disco.
sidebar:
  order: 1
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: 0ec92808eb43
---

## Despachar trabajo

Di, o escribe, lo que quieres que se haga: *"continúa la migración del webhook"*. Hark lo resuelve contra las sesiones en tu disco por coincidencia de palabras completas (nunca subcadenas, así que "core" no coincide con "hark-core"), retoma la ganadora en su propio espacio de trabajo con todos tus ajustes y transmite cada evento de vuelta a la ventana. Un término raro pesa más que uno común; un empate cerrado aparece como candidatos en lugar de una adivinanza.

Antes de que arranque un worker caro, un evaluador barato comprueba la instrucción contra la sesión en la que está por caer: unos centavos para evitar dólares gastados en la conversación equivocada. Las sesiones pesadas reciben primero un aviso honesto ("historial de 3.8 MB") con la opción de compactar antes de enviar. Los workers corren en paralelo entre proyectos, y cada turno, incluidos los que fallan, cae en el libro de cuentas.

## El tablero

Un kanban de tareas, en Início junto a Hoje y Demandas (la puerta Code guarda un acceso directo que lo abre como panel junto al chat). Una **tarea es un registro deliberado**: nada crea una por ti. Nace cuando la pides ("crear tarea arreglar el dns"), cuando aceptas la oferta del propio chat (tras unos turnos en un chat sin tarjeta, una línea sobre el compositor ofrece registrarla, una sola vez), o en el tablero mismo. Una tarea queda enlazada al chat donde se pidió; un chat puede llevar varias tareas, y una tarea puede reunir varios chats. Mover una tarjeta sigue las conversaciones que hacen el trabajo, así que retomar significa retomar, no empezar de cero. Un plan aprobado se convierte en la lista de verificación de la tarjeta; el trabajo terminado baja a hecho por sí solo.

## Ventanas de proyecto

El tablero es global; una ventana de proyecto separada muestra su parte, más lo que el proyecto ha gastado.
