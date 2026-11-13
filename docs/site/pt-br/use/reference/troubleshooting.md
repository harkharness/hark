---
title: Solução de problemas
description: Gatekeeper, o microfone, o atalho, um agente ausente, o modelo de fala, histórico, custos, desinstalação.
sidebar:
  order: 3
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: ef126386ccad
---

## "hark.app está danificado e não pode ser aberto"

O macOS põe em quarentena tudo o que é baixado da internet, e o Hark ainda não é assinado. O instalador limpa a flag; se você baixou o `.dmg` à mão:

```bash
xattr -cr /Applications/hark.app
```

Se o Gatekeeper ainda recusar, clique com o botão direito no app, Abrir uma vez, e o macOS lembra a exceção.

## O microfone não faz nada

O assistente de primeira execução grava dois segundos de propósito, o que faz o macOS pedir permissão com contexto. Se você pulou ou negou o pedido, conceda em Ajustes do Sistema › Privacidade e Segurança › Microfone, depois reinicie o Hark. Nada é gravado a menos que você pressione o atalho, e o áudio nunca sai da máquina.

## O atalho não abre nada

Outro app é dono do mesmo atalho. Mude em Configurações, ou na configuração:

```toml
hotkey = "cmd+shift+h"
```

## "agent plugin not found"

O Hark precisa de pelo menos um CLI de agente instalado e logado; ele não traz nenhum e não consegue autenticar por você. Confira que ele funciona sozinho primeiro:

```bash
claude --version
gemini --version
codex-acp --version
```

Se o binário está em um lugar incomum, aponte o Hark para ele (`claude_bin`, ou o `cmd` da entrada do agente), depois Configurações › Plugins › verificar de novo.

## O catálogo diz que meu agente está atrasado

Configurações › Plugins compara a versão que o seu binário relata com a que o registro ACP publica e mostra a linha de atualização. Um Gemini CLI antigo responde a toda sessão nova com "This client is no longer supported"; isso é a atualização, não um problema de login.

## O agente diz que não está logado

O cartão traz o comando de login do próprio agente. Faça login uma vez no seu terminal e envie a mensagem de novo; o Hark nunca digita credenciais por você.

## O modelo de fala falhou ao baixar

São cerca de 466 MB via `curl`, com checksum na chegada. Tente de novo pelas Configurações, ou pelo terminal:

```bash
hark setup
```

## Sem histórico de sessões na barra lateral

O Hark indexa os logs do próprio CLI do agente. Se o diretório está em outro lugar, defina `projects_dir` e reconstrua o índice:

```bash
hark index
```

## O painel de custos mostra tokens mas não dólares

USD só vem do relato do próprio agente; o Hark não tem tabela de preços. Um turno que ninguém precificou mostra `$ –` com o motivo ao passar o mouse.

## Desinstalar

```bash
rm -rf /Applications/hark.app
rm -f  ~/.local/bin/hark
rm -rf ~/.hark
```

O último apaga sua configuração, índice, livro-razão, quadro, o arquivo de memória do assistente e os registros de sessão do próprio Hark. Se você instalou a ponte da linha de status, desinstale-a pelas Configurações antes, para que as configurações do seu CLI de agente sejam restauradas.

## No Linux

O `.deb` pertence ao `apt`: rode o comando de instalação de novo para atualizar, e `sudo apt remove hark` para desinstalar. O AppImage se atualiza sozinho como o app do Mac; ele precisa de FUSE 2 para abrir na maioria das distros (`libfuse2` no Ubuntu e no Debian, `fuse2` nas outras), ou rode-o com `--appimage-extract-and-run`. A voz fala pelo Speech Dispatcher: se o Hark fica mudo, confira se o serviço responde sozinho, e escolha uma voz da lista dele em Configurações:

```bash
spd-say -l pt-BR "olá"
spd-say -L
```

O microfone vem pelo PipeWire ou pelo PulseAudio; uma sessão de desktop tem um dos dois. Numa máquina virtual, uma janela em branco costuma ser o compositor do WebKitGTK brigando com a GPU virtual: abra o Hark com `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

## Outra coisa

[Abra uma issue](https://github.com/harkharness/hark/issues) com sua versão do macOS, sua arquitetura e a release que você instalou. O "Reportar um problema" do app (barra lateral, Configurações ou o menu) preenche isso por você, com diagnósticos limpos se você marcar a caixa: a issue é pública, leia a prévia.
