---
title: Instalar
description: Un comando instala la app y el CLI; o descarga un .dmg de la última release.
sidebar:
  order: 2
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: cdd2cbb08a8f
---

Un comando elige el build correcto para tu Mac, instala el CLI `hark` en `~/.local/bin`, deja `hark.app` en `/Applications` y quita el bit de cuarentena (la app todavía no está firmada, así que macOS se negaría a abrirla):

```bash
curl -fsSL https://harkharness.web.app/install.sh | bash
```

## ¿Prefieres hacer clic?

Descarga un `.dmg` de [la última release](https://github.com/harkharness/hark/releases/latest):

| Archivo | Para |
|---|---|
| `hark-app-aarch64-apple-darwin.dmg` | macOS, Apple Silicon (M1 y más recientes) |
| `hark-app-x86_64-apple-darwin.dmg` | macOS, Intel |
| `hark-cli-<target>.tar.gz` | solo el CLI `hark`, sin interfaz |

Si descargas el `.dmg` a mano, ejecuta esto una vez antes de abrir la app:

```bash
xattr -cr /Applications/hark.app
```

## Compilar desde el código fuente

Necesitas las herramientas de línea de comandos de Xcode, Rust (stable; CI fija 1.98.0), Node 22 y npm.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev                     # la app de escritorio, con recarga en caliente
cargo build --release -p hark-cli     # el CLI sin interfaz
```

La [documentación para contribuidores](/docs/es/build/contributing/) cubre el resto: las pruebas, la estructura del proyecto y cómo se corta una release.

## Actualizaciones

Cuando sale una nueva release, la app la descarga en segundo plano, verifica su firma y muestra una sola píldora discreta en la ventana principal: **reiniciar para actualizar**. Haz clic y la app se reabre en la versión nueva; ignórala y no pasa nada. La comprobación es un único GET a las releases de GitHub, la única llamada de red que Hark hace por su cuenta. Desactívala con `auto_update = false` y actualiza a mano con el mismo comando de instalación. El CLI `hark` lo actualiza ese script; el actualizador de la app reemplaza solo el paquete de la app.
