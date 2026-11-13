---
title: Solución de problemas
description: Gatekeeper, el micrófono, el atajo, un agente que falta, el modelo de voz, el historial, los costos, desinstalar.
sidebar:
  order: 3
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: ef126386ccad
---

## "hark.app está dañado y no se puede abrir"

macOS pone en cuarentena todo lo descargado de internet, y Hark todavía no está firmado. El instalador quita la marca; si descargaste el `.dmg` a mano:

```bash
xattr -cr /Applications/hark.app
```

Si Gatekeeper sigue negándose, haz clic derecho en la app, Abrir una vez, y macOS recuerda la excepción.

## El micrófono no hace nada

El asistente de la primera ejecución graba dos segundos a propósito, lo que hace que macOS pida permiso con contexto. Si lo saltaste o rechazaste el aviso, concédelo en Ajustes del Sistema › Privacidad y seguridad › Micrófono, y luego reinicia Hark. No se graba nada salvo que presiones el atajo, y el audio nunca sale de la máquina.

## El atajo no abre nada

Otra app tiene el mismo atajo. Cámbialo en Ajustes, o en la configuración:

```toml
hotkey = "cmd+shift+h"
```

## "agent plugin not found"

Hark necesita al menos un CLI de agente instalado y con sesión iniciada; no incluye ninguno y no puede autenticarse por ti. Comprueba primero que funciona por su cuenta:

```bash
claude --version
gemini --version
codex-acp --version
```

Si el binario vive en un lugar inusual, apunta Hark a él (`claude_bin`, o el `cmd` de la entrada del agente), y luego Ajustes › Plugins › volver a comprobar.

## El catálogo dice que mi agente está desactualizado

Ajustes › Plugins compara la versión que reporta tu binario con la que publica el registro ACP y muestra la línea de actualización. Un Gemini CLI viejo responde a cada sesión nueva con "This client is no longer supported"; eso es la actualización, no un problema de inicio de sesión.

## El agente dice que no tiene sesión iniciada

La tarjeta trae el comando de inicio de sesión de ese agente. Inicia sesión una vez en tu terminal y envía el mensaje de nuevo; Hark nunca escribe credenciales por ti.

## El modelo de voz no se pudo descargar

Son unos 466 MB por `curl`, con checksum al llegar. Reintenta desde Ajustes, o desde la terminal:

```bash
hark setup
```

## No hay historial de sesiones en la barra lateral

Hark indexa los logs propios del CLI del agente. Si el directorio está en otro lugar, define `projects_dir` y reconstruye el índice:

```bash
hark index
```

## El panel de costos muestra tokens pero no dólares

Los USD solo vienen del reporte del propio agente; Hark no tiene tabla de precios. Un turno al que nadie puso precio muestra `$ –` con la razón al pasar el cursor.

## Desinstalar

```bash
rm -rf /Applications/hark.app
rm -f  ~/.local/bin/hark
rm -rf ~/.hark
```

El último borra tu configuración, el índice, el libro de cuentas, el tablero, el archivo de memoria del asistente y los registros de sesión propios de Hark. Si instalaste el puente de la línea de estado, desinstálalo primero desde Ajustes para que los ajustes de tu CLI de agente queden restaurados.

## En Linux

El `.deb` pertenece a `apt`: ejecuta el comando de instalación otra vez para actualizar, y `sudo apt remove hark` para desinstalar. El AppImage se actualiza solo como la app del Mac; necesita FUSE 2 para abrir en la mayoría de las distros (`libfuse2` en Ubuntu y Debian, `fuse2` en las demás), o ejecútalo con `--appimage-extract-and-run`. La voz habla a través de Speech Dispatcher: si Hark se queda mudo, comprueba que el servicio responde por sí solo, y elige una voz de su lista en Configuración:

```bash
spd-say -l es-ES "hola"
spd-say -L
```

El micrófono llega por PipeWire o PulseAudio; una sesión de escritorio tiene uno de los dos. En una máquina virtual, una ventana en blanco suele ser el compositor de WebKitGTK peleando con la GPU virtual: abre Hark con `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

## Algo más

[Abre un issue](https://github.com/harkharness/hark/issues) con tu versión de macOS, tu arquitectura y la release que instalaste. El "Reportar un problema" de la app (barra lateral, Ajustes o el menú) lo rellena por ti, con diagnósticos limpiados si marcas la casilla: el issue es público, lee la vista previa.
