---
title: Instalar
description: Um comando instala o app e o CLI; ou pegue um .dmg na última release.
sidebar:
  order: 2
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: cdd2cbb08a8f
---

Um comando escolhe o build certo para o seu Mac, instala o CLI `hark` em `~/.local/bin`, coloca o `hark.app` em `/Applications` e limpa o bit de quarentena (o app ainda não é assinado, então o macOS se recusaria a abri-lo):

```bash
curl -fsSL https://harkharness.web.app/install.sh | bash
```

## Prefere clicar?

Pegue um `.dmg` na [última release](https://github.com/harkharness/hark/releases/latest):

| Arquivo | Para |
|---|---|
| `hark-app-aarch64-apple-darwin.dmg` | macOS, Apple Silicon (M1 e mais novos) |
| `hark-app-x86_64-apple-darwin.dmg` | macOS, Intel |
| `hark-cli-<target>.tar.gz` | só o CLI `hark`, sem interface |

Se você baixar o `.dmg` à mão, rode isto uma vez antes de abrir o app:

```bash
xattr -cr /Applications/hark.app
```

## Compilar do código-fonte

Você precisa das ferramentas de linha de comando do Xcode, Rust (stable; a CI fixa 1.98.0), Node 22 e npm.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev                     # o app desktop, com hot reload
cargo build --release -p hark-cli     # o CLI sem interface
```

A [documentação de contribuidor](/docs/pt-br/build/contributing/) cobre o resto: testes, a organização do projeto e como uma release é cortada.

## Atualizações

Quando uma release nova sai, o app a baixa em segundo plano, verifica a assinatura e mostra uma pílula discreta na janela principal: **reiniciar para atualizar**. Clique e o app reabre na versão nova; ignore e nada acontece. A verificação é um único GET nas releases do GitHub, a única chamada de rede que o Hark faz por conta própria. Desligue com `auto_update = false` e atualize à mão com o mesmo comando de instalação. O CLI `hark` é atualizado por esse script; o atualizador do app substitui só o bundle do app.
