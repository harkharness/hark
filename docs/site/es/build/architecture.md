---
title: Estructura del proyecto y arquitectura
description: Hexagonal, con un núcleo funcional y una cáscara imperativa; dónde vive cada cosa.
sidebar:
  order: 2
source_hash: 9ded079f1458
---

Hark es hexagonal (puertos y adaptadores), con un núcleo funcional y una cáscara imperativa:

```
crates/hark-core           toda la lógica, sin UI
  src/domain/              funciones puras y tipos inmutables, sin I/O
  src/ports/               traits: Stt, Tts, AgentRunner, SessionStore, SpendLedger…
  src/adapters/            cáscaras delgadas: sqlite, whisper, cpal, say, pty
crates/hark-agent          el contrato de plugin: un vocabulario de eventos + una hoja de capacidades
crates/hark-plugin-claude  el backend de Claude Code
crates/hark-plugin-acp     todo backend Agent Client Protocol
crates/hark-cli            driver sin interfaz: hark ask / dispatch / spend
src-tauri + src/           la app de escritorio (Tauri v2 + React)
```

El código del dominio se escribe con pruebas primero y corre sin micrófono, base de datos ni red. La canalización de voz es local: whisper.cpp (Metal en Apple Silicon, CPU en el resto) para voz a texto, la voz del sistema para texto a voz.

## Tres capas de respuesta

Las preguntas deterministas (el tablero, los workers en curso, el gasto de hoy) se responden en local por cero tokens. Las preguntas que solo necesitan reformular reciben un modelo ligero con cerca de 1k de contexto. Solo el razonamiento real paga precio completo. Un evaluador barato filtra un despacho antes de que un worker queme dólares en la sesión equivocada.

## Todo local primero

El historial de sesiones vive en un índice SQLite construido de forma incremental a partir de los logs propios del CLI. Recuperar un chat, buscar sesiones, abrir archivos y leer transcripciones nunca toca un modelo. Cada turno corre bajo un techo de dólares sostenido por Hark mismo.

## Dónde viven las notas para agentes

El repositorio guarda las notas escritas para agentes y para mantenedores: `CLAUDE.md`, la skill de artefactos que Hark instala, y `docs/FINDINGS*.md`, los hallazgos medidos detrás de la arquitectura (costos por camino, lo que cada CLI reporta). Las páginas que estás leyendo salen de `docs/site/` en el mismo repositorio.
