---
title: La barra de sesión
description: Entorno, contexto, conectores, salidas, cambios, tareas en segundo plano, costos y límites.
sidebar:
  order: 3
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: f98207961b40
---

La barra de sesión se lee como la de la app de Claude Code: secciones que se pliegan y lo recuerdan, cada una con su resumen mientras está plegada.

| Sección | Qué muestra |
|---|---|
| **Entorno** | esta máquina, el proyecto y su carpeta, la rama, el agente y sus controles |
| **Contexto** | la parte de la ventana ocupada, y qué la llena |
| **Conectores** | los servidores MCP que la sesión abrió, con su estado; reconectar está en Ajustes › Plugins |
| **Salidas** | lo que la sesión escribió: una página bajo `~/.hark/artifacts` por su título y tipo, que se abre en el panel de artefactos; un markdown del espacio de trabajo como documento, cualquier otro archivo como archivo, ambos abiertos en la ventana de archivos |
| **Cambios** | el git de la sesión: cada archivo cambiado desde el último commit, las líneas del que abres, una marca por archivo que ya leíste, y "hacer commit de los cambios", que entrega la petición al chat. Hark no hace commit de nada por sí mismo |
| **Tareas en segundo plano** | comandos que el agente dejó corriendo, y cuántos terminaron |
| **Costos** y **Límites** | lo que costó esta sesión, y las ventanas de la suscripción |

El diff vive aquí y en ningún otro lugar: es por sesión.
