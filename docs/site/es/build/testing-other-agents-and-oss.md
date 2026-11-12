---
title: Probar otros agentes y sistemas operativos
description: La mayor parte de Hark se probó con Claude Code en macOS. Así puedes ayudar con el resto.
sidebar:
  order: 5
source_hash: 4d797b816b55
---

La mayor parte de las pruebas hasta ahora se hizo con el CLI de Claude Code en macOS, Apple Silicon e Intel. Gemini CLI, Codex y los demás agentes ACP corren, y se publican, pero han tenido muchas menos horas. Linux y Windows son las próximas releases y todavía no son usables. Aquí es donde la ayuda importa más.

## Probar otro agente

1. Instala el CLI del agente e inicia sesión con él en tu terminal primero.
2. Abre Ajustes › Plugins: la tarjeta muestra si Hark lo detectó, la versión frente al registro y lo que el plugin dice que puede hacer.
3. Abre un chat en él ("abre un chat en gemini en webhook"), envía unos turnos, provoca un permiso, deja que un turno ejecute un comando de shell y mira el libro de cuentas.
4. Reporta lo que no coincidió con la documentación: [abre un issue](https://github.com/harkharness/hark/issues) con el agente y su versión, tu sistema operativo y arquitectura, la release de Hark y los diagnósticos de la app ("Reportar un problema" los adjunta, limpiados).

Lo que más queremos saber: si los permisos llegaron como tarjetas, si las píldoras de modelo y esfuerzo hablaron correctamente al agente, si el libro de cuentas mostró tokens y, donde el agente lo reporta, dólares, y si un chat retomado realmente se retomó.

## Probar otro sistema operativo

Los ports se siguen en el repositorio. Lo que se sabe que hoy es solo macOS: texto a voz (`say`), los sonidos de aviso, el build Metal del modelo de voz, abrir archivos en el Finder, la barra de título superpuesta de la ventana, los atajos (`Cmd`) y el instalador. Si puedes compilar desde el código fuente en Linux o Windows, el reporte más útil es lo primero que se rompe, con el log del build.

## Reportar

Cada reporte es público; lee lo que contienen los diagnósticos antes de enviar. Nada sale de tu máquina a través de Hark mismo.
