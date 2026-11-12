---
title: Contribuir
description: Buenas primeras contribuciones, la configuración, las comprobaciones y cómo se escribe el código aquí.
sidebar:
  order: 1
source_hash: 01b84c61567e
---

Gracias por ayudar. Hark tiene una forma clara, una cabina sobre los CLI de agentes que la gente ya paga, local primero, honesta con el costo, y las mejores contribuciones conservan esa forma. Para cualquier cosa mayor que un arreglo, abre primero un issue para que acordemos la forma antes de que gastes un fin de semana en ello.

## Buenas primeras contribuciones

- **Un agente que habla ACP.** La mayoría de los agentes nuevos son una entrada en `builtins()` (`crates/hark-core/src/domain/agents.rs`), con su `cmd`, `args`, archivo de memoria y niveles de modelo, más una línea en la documentación.
- **Un backend para un CLI que no habla ACP.** Empieza por [el contrato de plugin](/docs/es/build/plugin-contract/).
- **Probar Hark en otro agente o sistema.** Lo más valioso ahora mismo: ver [Probar otros agentes y sistemas operativos](/docs/es/build/testing-other-agents-and-oss/).
- **Traducciones**, de la app y de esta documentación: ver [Traducciones](/docs/es/build/translations/).
- **Frases de voz.** `crates/hark-core/tests/corpus/voice_corpus.jsonl` mantiene honesto al clasificador de voz: una frase, el catálogo contra el que se dijo y dónde debería caer.
- **Reportes de errores** con la versión de Hark, la versión del sistema operativo y la arquitectura, y el CLI del agente con su versión. El "Reportar un problema" de la app lo rellena por ti.

## Configuración

Necesitas macOS 12+, las herramientas de línea de comandos de Xcode, Rust (stable; CI fija 1.98.0), Node 22 y npm, y al menos un CLI de agente con sesión iniciada.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev
```

La primera ejecución descarga el modelo de voz (unos 500 MB) y pide el micrófono. Para ver la ventana sin backend, y para tomar capturas: `npm run dev`, luego `http://localhost:1420/?demo=home`; ver [Frontend](/docs/es/build/frontend/).

## Comprobaciones

Todo lo que corre CI, ejecutable en local:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npx tsc --noEmit
npm test
npm run build
npm run docs:check
```

## Cómo se escribe el código aquí

- **Núcleo funcional, cáscara imperativa.** `crates/hark-core/src/domain` es puro: sin I/O, tipos inmutables, probado sin micrófono, base de datos ni red. Los adaptadores y la capa Tauri se mantienen delgados.
- **Primero las pruebas en el dominio.** Escribe la prueba, mírala fallar por la razón que esperas, y luego hazla pasar.
- **La salida del agente no es confiable.** Un repositorio puede inyectar un prompt y hacer que un agente imprima cualquier cosa. El código que convierte texto del agente en una acción (abrir un archivo, escribir en disco, la terminal, lanzar un proceso) pasa por una guarda con prueba. `SECURITY.md` tiene el modelo.
- **Los datos de ejemplo son inventados.** Fixtures, mocks, capturas y ejemplos nunca llevan nombres reales de empresas, clientes, tickets o personas.
- **Los commits** van en inglés, `type(scope): summary` en minúsculas (`fix`, `feat`, `chore`, `ci`, `docs`), con un cuerpo que explica el porqué.

## Licencia

Al contribuir aceptas que tu contribución se licencie bajo MIT o Apache-2.0, como el resto de Hark, sin términos ni condiciones adicionales.
