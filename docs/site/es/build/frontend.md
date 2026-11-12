---
title: Frontend y gramática visual
description: React 18 + TypeScript + Vite; un orquestador, componentes puros, un modo demo para capturas.
sidebar:
  order: 4
source_hash: 1c0fefe7b1fe
---

React 18 + TypeScript + Vite, solo componentes funcionales. La estructura refleja la filosofía del lado Rust: renderizado puro en los bordes, estado y efectos secundarios concentrados en un orquestador (`src/App.tsx`). `src/lib/ipc.ts` es el único lugar donde se invoca un comando de Tauri, `src/lib/events.ts` el único donde se escuchan eventos, `src/lib/*.ts` las reglas puras (alcance, búsqueda, diff, salidas, artefactos), `src/components/` la presentación, con el estado entrando por props.

## Reglas

1. **Los componentes nunca llaman a `invoke`.** Todo pasa por `lib/ipc.ts`, así que los nombres de comandos y los payloads tienen un solo hogar.
2. **El estado vive en App.** Los componentes reciben datos y callbacks; el estado transitorio de UI (un menú abierto, un borrador) puede quedarse local, cualquier cosa que otro componente lee vive en App.
3. **Los hilos se indexan por la etiqueta de la tarea**, no por su id, porque la transcripción sobrevive a los reinicios del worker. Una línea leída desde el archivo de sesión se descarta cuando el hilo ya la muestra.
4. **Los eventos del backend llegan solo por `useHarkEvents`.** Un tipo de evento nuevo recibe una rama ahí y una variante en `types.ts`.
5. **Las funciones de cero tokens** (visor de archivos, apertura rápida, acciones del tablero) nunca lanzan un proceso de agente. Si una función cuesta dinero, pasa por los flujos de compuerta y enrutador en App.
6. **La ventana corre bajo una CSP.** Scripts solo desde el bundle; el HTML de los artefactos se muestra en un `<iframe sandbox>` que no agrega ningún origen.

## Modo demo

`npm run dev`, luego `http://localhost:1420/?demo=<scene>` en cualquier navegador: cada comando de Tauri se responde desde `lib/demo.ts` y cada evento viene del guion de la escena, así que la ventana corre sin backend. Las escenas son `home`, `inicio`, `task`, `board`, `docs` y `costs`. Las capturas del README y del sitio se toman aquí, a 1440×900 y 2×; nada en ellas es un proyecto, una sesión o un nombre real. `demo.test.ts` falla cuando un comando que la costura conoce no tiene respuesta en el demo.

## Agregar un comando de Tauri

1. Impleméntalo en `src-tauri/src/lib.rs` y regístralo en `invoke_handler`.
2. Agrega un wrapper tipado en `src/lib/ipc.ts`.
3. Agrega o extiende los tipos del payload en `src/types.ts` (los nombres de campo se quedan en snake_case, como se serializan).

## La gramática visual

`docs/CHROME.md` en el repositorio es la gramática visual: medir, no mirar; tokens en `:root` (una escala tipográfica de seis tamaños, cuerpo a 14px; colores; bordes finos); separar por lo que hace el contenido, no dibujando un límite; los divisores son líneas finas con un área de agarre; una columna, un margen interior; cada ventana tiene la barra de título superpuesta; dos puertas, una barra lateral; encabezados de panel; y las trampas de la cascada que hicieron que varias de estas parecieran decisiones de diseño cuando no lo eran. Títulos, encabezados, elementos de menú, botones, placeholders y tooltips empiezan con mayúscula.
