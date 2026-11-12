---
title: Permisos
description: Tarjetas en línea que respondes por clic, teclado o voz, y un piso que no se mueve.
sidebar:
  order: 4
banner:
  content: "<b>Hark está en desarrollo.</b> La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS. ¿Usas otro agente u otro sistema? <a href=\"/docs/es/build/testing-other-agents-and-oss/\">Ayúdanos a probarlo.</a>"
source_hash: f0fabf7fd5f1
---

Cada llamada a una herramienta privilegiada se muestra como una tarjeta en línea en el hilo: la herramienta, el payload y permitir / siempre / denegar. Responde por clic, por teclado o por voz desde cualquier ventana ("sí", "no"). Nada bloquea el resto de la interfaz: un permiso esperando en un hilo nunca congela otro.

## El piso duro

Los comandos que tocan producción (`kubectl apply`, `terraform apply`, `helm`, force push, SQL destructivo, borrados en la nube, publicación de paquetes) nunca se aprueban automáticamente, sea cual sea el agente que los ejecute. Las reglas permanentes de "permitir siempre" se ignoran para ellos, la tarjeta se pone roja y, en modo bypass, esos CLI se bloquean por completo en Claude Code. Ese bloqueo es un flag de Claude Code sin equivalente en otros agentes, así que en cualquier otro agente elegir bypass abre el hilo en accept-edits y lo dice en el chat. Hark puede ser descuidado con tus tokens; no tiene permitido ser descuidado con tu infraestructura.

## Modos

La píldora de permisos junto al compositor fija la postura con la que empiezan los turnos nuevos: manual (preguntar por todo), aceptar ediciones, plan, auto, bypass. `worker_mode` en la [configuración](/docs/es/use/reference/configuration/) fija el valor predeterminado. Ningún modo desactiva el piso.
