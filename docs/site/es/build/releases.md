---
title: Releases
description: Cortadas desde una pipeline privada que guarda la clave de firma; una release de funciones por semana.
sidebar:
  order: 7
source_hash: 8fd1b303770b
---

Los mantenedores cortan las releases desde una pipeline privada que guarda la clave de firma. Compila exactamente el código etiquetado en el repositorio público, firma los artefactos del actualizador, verifica cada firma y publica en las Releases del repositorio; las apps instaladas solo aceptan actualizaciones firmadas con esa clave.

## Las reglas

- **Como mucho una release de funciones por semana.** Una release de corrección (un salto de patch, mismo major.minor) está exenta.
- **La etiqueta debe coincidir con la versión de la app** en `src-tauri/tauri.conf.json` y `Cargo.toml`; la pipeline sube ambas cuando corta.
- **La documentación debe estar al día**: la compuerta de traducción corre antes de cada build.
- **Los nombres de los assets no llevan versión**, así que los enlaces `releases/latest/download/<name>` siguen funcionando para siempre.

## Lo que lleva una release

Un `.dmg` y un tarball firmado para el actualizador, para Apple Silicon y para Intel, el CLI `hark` para ambos, checksums, y `latest.json` para el actualizador de la app. Hacer push a `main` nunca publica nada: la release es un disparo deliberado y manual.
