---
title: Releases
description: Cortadas de um pipeline privado que guarda a chave de assinatura; uma release de funcionalidade por semana.
sidebar:
  order: 7
source_hash: 8fd1b303770b
---

Os mantenedores cortam releases de um pipeline privado que guarda a chave de assinatura. Ele compila exatamente a fonte com tag no repositório público, assina os artefatos do atualizador, verifica cada assinatura e publica nas Releases do repositório; apps instalados só aceitam atualizações assinadas por essa chave.

## As regras

- **No máximo uma release de funcionalidade por semana.** Uma release de correção (um patch, mesmo major.minor) está isenta.
- **A tag precisa bater com a versão do app** em `src-tauri/tauri.conf.json` e `Cargo.toml`; o pipeline sobe as duas quando corta.
- **A documentação precisa estar em dia**: o gate de tradução roda antes de um build.
- **Nomes de asset não carregam versão**, então links `releases/latest/download/<name>` continuam funcionando para sempre.

## O que uma release carrega

Um `.dmg` e um tarball assinado do atualizador para Apple Silicon e para Intel, o CLI `hark` para os dois, checksums, e `latest.json` para o atualizador do app. Push em `main` nunca lança nada: a release é um disparo deliberado, manual.
