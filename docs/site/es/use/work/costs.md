---
title: Costos y límites
description: Un libro de cuentas local de cada turno, honesto sobre lo que no sabe.
sidebar:
  order: 2
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: e2074efec1da
---

Cada turno cae en un libro de cuentas local en SQLite: tipo, proyecto, tarea, modelo, tokens, costo, si falló. Ajustes › Costos lo desglosa por día o semana, por tipo, proyecto, tarea y modelo, muestra tu tasa de aciertos de caché, lo que la arquitectura evitó gastar, y lista las sesiones más caras.

## Dos reglas mantienen los números honestos

- **Los USD vienen solo del reporte del propio agente.** No hay tabla de precios en ningún lugar de Hark. Claude Code y el adaptador ACP de Claude ponen precio a sus turnos; Codex reporta solo tokens; Gemini no reporta nada. Un turno al que nadie puso precio se muestra como `$ –`, nunca como `$0.00`.
- **Los conteos de tokens y los dólares nunca se suman entre sí.** Vienen de fuentes distintas, el stream en vivo y los logs de sesión, y los logs cubren todo tu uso del agente, no solo lo que pasó por Hark.

## El techo

Cada turno corre bajo un techo de dólares (`worker_budget_usd`, 5 por defecto) sostenido por Hark mismo: pone precio a las llamadas del turno a medida que el stream las reporta y corta el turno entre llamadas al pasar el techo, luego dice cuánto, y ofrece subirlo para ese chat, reiniciar en ligero o cambiar de modelo. En todos los agentes.

## El camino barato es el predeterminado

Las preguntas cortas y completas sobre los workers en curso, el tablero, el gasto y la lista del día se responden desde el índice local por cero tokens. Las preguntas que solo necesitan reformular reciben un modelo ligero con un contexto pequeño. Solo el razonamiento real paga precio completo. Di "piensa más" para subir de nivel o "una rápida" para bajar, por pregunta.

## Ventanas de la suscripción

Los porcentajes de las ventanas de 5h y 7d vienen de un puente opcional sobre la línea de estado de Claude Code (Ajustes › Costos), o del adaptador ACP de Claude, que los reporta en cada turno.
