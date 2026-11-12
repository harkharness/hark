---
title: Plugins y cambio de agente
description: El catálogo en Ajustes, qué agente abre los chats nuevos y cómo mover un hilo entre agentes.
sidebar:
  order: 3
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: 4b108c388809
---

Ajustes › **Plugins** es el catálogo: cada agente que Hark conoce, si su binario está en esta máquina y la línea de instalación cuando no, lo que cada plugin puede hacer (retomar, permisos, reporte de costos, historial, comandos slash…), cómo llama el agente a cada nivel de modelo, qué build tienes frente al que publica el registro ACP, y cuál abre los chats nuevos.

## Una cabina, varios agentes

Cada chat pertenece al agente que lo abrió, y las píldoras junto al compositor (modo de permisos, nivel de modelo, esfuerzo) hablan a ese agente en su propio vocabulario. Un control que un agente no ofrece se queda en pantalla, desactivado, con la razón: oculto se leería como "Hark no puede".

Dos comandos hablados mueven entre agentes:

- *"abre un chat en gemini en webhook"*: un chat nuevo en ese proyecto, en ese agente.
- *"cambia este chat a gemini"*: el hilo en el que estás se mueve. Una sesión nueva en el otro agente se abre con un brief local, de cero tokens, de la conversación hasta ahora, en la misma tarjeta. El popover ⓘ de un hilo activo tiene un botón por agente disponible para el mismo movimiento.

## Conectores

La sección *Conectores* de la barra de sesión lista los servidores MCP que la sesión abrió, con su estado. Reconectar está en Ajustes › Plugins.
