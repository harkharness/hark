---
title: Instalar
description: Um comando instala o app e o CLI; ou pegue um .dmg na última release.
sidebar:
  order: 2
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 9c2bf7673b4c
---

Um comando escolhe o build certo para a sua máquina, instala o CLI `hark` em `~/.local/bin` e o app onde o seu sistema guarda apps:

```bash
curl -fsSL https://harkharness.web.app/install.sh | bash
```

No macOS, é o `hark.app` em `/Applications`, com o bit de quarentena limpo (o app ainda não é assinado, então o macOS se recusaria a abri-lo). No Ubuntu, no Debian e nos parentes deles, é o `.deb`, instalado pelo `apt` para que os pacotes do WebKit, do GTK e do Speech Dispatcher venham junto (o sudo pede a sua senha uma vez). Em qualquer outro Linux, é o AppImage em `~/.local/opt/hark`, com uma entrada no lançador.

## Prefere clicar?

Pegue um pacote na [última release](https://github.com/harkharness/hark/releases/latest):

| Arquivo | Para |
|---|---|
| `hark-app-aarch64-apple-darwin.dmg` | macOS, Apple Silicon (M1 e mais novos) |
| `hark-app-x86_64-apple-darwin.dmg` | macOS, Intel |
| `hark-app-x86_64-unknown-linux-gnu.deb` | Ubuntu 22.04+, Debian 12+ e parentes, x86_64 |
| `hark-app-aarch64-unknown-linux-gnu.deb` | os mesmos, em arm64 |
| `hark-app-<target>.AppImage` | qualquer outro Linux (precisa de FUSE 2: `libfuse2`) |
| `hark-cli-<target>.tar.gz` | só o CLI `hark`, sem interface |

Se você baixar o `.dmg` à mão, rode isto uma vez antes de abrir o app:

```bash
xattr -cr /Applications/hark.app
```

Um `.deb` instala com `sudo apt install ./hark-app-<target>.deb`; um AppImage só precisa do bit de execução.

## Compilar do código-fonte

Você precisa de Rust (stable; a CI fixa 1.98.0), Node 22 e npm. No macOS, as ferramentas de linha de comando do Xcode; no Linux, os pacotes de desenvolvimento do WebKitGTK, do GTK, do ALSA e do Speech Dispatcher: o `scripts/dev-linux.sh` instala todos eles no Ubuntu e roda o app.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev                     # o app desktop, com hot reload
cargo build --release -p hark-cli     # o CLI sem interface
```

A [documentação de contribuidor](/docs/pt-br/build/contributing/) cobre o resto: testes, a organização do projeto e como uma release é cortada.

## Atualizações

Quando uma release nova sai, o app a baixa em segundo plano, verifica a assinatura e mostra uma pílula discreta na janela principal: **reiniciar para atualizar**. Clique e o app reabre na versão nova; ignore e nada acontece. A verificação é um único GET nas releases do GitHub, a única chamada de rede que o Hark faz por conta própria. Desligue com `auto_update = false` e atualize à mão com o mesmo comando de instalação. Uma instalação por `.deb` pertence ao `apt`: ali o app nunca se substitui, então o comando de instalação é o jeito de atualizar. O CLI `hark` é atualizado por esse script; o atualizador do app substitui só o bundle do app.
