---
title: Organização do projeto e arquitetura
description: Hexagonal, com núcleo funcional e casca imperativa; onde cada coisa vive.
sidebar:
  order: 2
source_hash: 9ded079f1458
---

O Hark é hexagonal (portas e adaptadores), com um núcleo funcional e uma casca imperativa:

```
crates/hark-core           toda a lógica, sem UI
  src/domain/              funções puras e tipos imutáveis, sem I/O
  src/ports/               traits: Stt, Tts, AgentRunner, SessionStore, SpendLedger…
  src/adapters/            cascas finas: sqlite, whisper, cpal, say, pty
crates/hark-agent          o contrato de plugin: um vocabulário de eventos + uma ficha de capacidades
crates/hark-plugin-claude  o backend do Claude Code
crates/hark-plugin-acp     todo backend Agent Client Protocol
crates/hark-cli            driver sem interface: hark ask / dispatch / spend
src-tauri + src/           o app desktop (Tauri v2 + React)
```

O código de domínio é guiado por testes e roda sem microfone, banco de dados ou rede. O pipeline de voz é local: whisper.cpp (Metal no Apple Silicon, CPU no resto) para fala em texto, a voz do sistema para texto em fala.

## Três camadas de resposta

Perguntas determinísticas (o quadro, workers rodando, o gasto de hoje) são respondidas localmente por zero tokens. Perguntas só de formulação vão a um modelo leve com cerca de 1k de contexto. Só raciocínio de verdade paga o preço cheio. Um avaliador barato filtra um despacho antes de um worker queimar dólares na sessão errada.

## Tudo local-first

O histórico de sessões vive em um índice SQLite construído incrementalmente a partir dos logs do próprio CLI. Lembrar um chat, pesquisar sessões, abrir arquivos e ler transcrições nunca tocam um modelo. Cada turno roda sob um teto em dólares mantido pelo próprio Hark.

## Onde vivem as notas para agentes

O repositório guarda as notas escritas para agentes e para mantenedores: `CLAUDE.md`, a skill de artefatos que o Hark instala, e `docs/FINDINGS*.md`, os achados medidos por trás da arquitetura (custos por caminho, o que cada CLI relata). As páginas que você está lendo vêm de `docs/site/` no mesmo repositório.
