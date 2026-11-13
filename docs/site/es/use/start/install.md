---
title: Instalar
description: Un comando instala la app y el CLI; o descarga un .dmg de la última release.
sidebar:
  order: 2
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: 9c2bf7673b4c
---

Un comando elige el build correcto para tu máquina, instala el CLI `hark` en `~/.local/bin` y la app donde tu sistema guarda las apps:

```bash
curl -fsSL https://harkharness.web.app/install.sh | bash
```

En macOS, es `hark.app` en `/Applications`, con el bit de cuarentena quitado (la app todavía no está firmada, así que macOS se negaría a abrirla). En Ubuntu, Debian y sus parientes, es el `.deb`, instalado con `apt` para que los paquetes de WebKit, GTK y Speech Dispatcher vengan con él (sudo pide tu contraseña una vez). En cualquier otro Linux, es el AppImage en `~/.local/opt/hark`, con una entrada en el lanzador.

## ¿Prefieres hacer clic?

Descarga un paquete de [la última release](https://github.com/harkharness/hark/releases/latest):

| Archivo | Para |
|---|---|
| `hark-app-aarch64-apple-darwin.dmg` | macOS, Apple Silicon (M1 y más recientes) |
| `hark-app-x86_64-apple-darwin.dmg` | macOS, Intel |
| `hark-app-x86_64-unknown-linux-gnu.deb` | Ubuntu 22.04+, Debian 12+ y parientes, x86_64 |
| `hark-app-aarch64-unknown-linux-gnu.deb` | los mismos, en arm64 |
| `hark-app-<target>.AppImage` | cualquier otro Linux (necesita FUSE 2: `libfuse2`) |
| `hark-cli-<target>.tar.gz` | solo el CLI `hark`, sin interfaz |

Si descargas el `.dmg` a mano, ejecuta esto una vez antes de abrir la app:

```bash
xattr -cr /Applications/hark.app
```

Un `.deb` se instala con `sudo apt install ./hark-app-<target>.deb`; un AppImage solo necesita el bit de ejecución.

## Compilar desde el código fuente

Necesitas Rust (stable; la CI fija 1.98.0), Node 22 y npm. En macOS, las herramientas de línea de comandos de Xcode; en Linux, los paquetes de desarrollo de WebKitGTK, GTK, ALSA y Speech Dispatcher: `scripts/dev-linux.sh` los instala todos en Ubuntu y ejecuta la app.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev                     # la app de escritorio, con hot reload
cargo build --release -p hark-cli     # el CLI sin interfaz
```

La [documentación de contribuidor](/docs/es/build/contributing/) cubre el resto: pruebas, la organización del proyecto y cómo se corta una release.

## Actualizaciones

Cuando sale una release nueva, la app la descarga en segundo plano, verifica la firma y muestra una píldora discreta en la ventana principal: **reiniciar para actualizar**. Haz clic y la app se reabre en la versión nueva; ignórala y no pasa nada. La comprobación es un único GET a las releases de GitHub, la única llamada de red que Hark hace por su cuenta. Desactívala con `auto_update = false` y actualiza a mano con el mismo comando de instalación. Una instalación por `.deb` pertenece a `apt`: ahí la app nunca se reemplaza a sí misma, así que el comando de instalación es la forma de actualizar. El CLI `hark` se actualiza con ese script; el actualizador de la app reemplaza solo el bundle de la app.
