---
title: Gemini CLI, Codex e qualquer agente ACP
description: "O plugin ACP: um protocolo, todo agente que o fala."
sidebar:
  order: 2
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: b69ed59a0638
---

O [Agent Client Protocol](https://agentclientprotocol.com) é o padrão aberto de editor para agente (JSON-RPC sobre stdio, um handshake de capacidades, permissões no fio) contra o qual Zed, JetBrains e Google publicam. O Hark o fala uma vez, e todo agente que o fala se encaixa: Gemini CLI (`gemini --acp`), Codex (`codex-acp`), Claude Code via ACP (`claude-agent-acp`, vem desligado), DeepSeek, Kiro, Google Antigravity e qualquer outro agente ACP com uma entrada `[agents.<id>]` na configuração. Cada um dos embutidos segue o [registro de agentes ACP](https://github.com/agentclientprotocol/registry) em como é distribuído e invocado.

## O que a mesma cabine dá a todo agente ACP

- **Permissões, sempre.** O mesmo cartão inline do Claude Code, respondido por clique, teclado ou voz. Permitir vale uma vez; regras permanentes são do próprio Hark.
- **Diretivas que atravessam.** As pílulas de modo, modelo e esforço falam com o agente nos ids dele, para o que ele ofereceu no início da sessão. A pílula de modelo fala em camadas, e a linha de registro de cada agente diz como ele as chama (`haiku` para o Claude, `gemini-2.5-flash-lite` para o Gemini). Um ajuste que o agente não oferece fica na tela, desabilitado, com o motivo.
- **Custos onde existem.** O adaptador do Claude precifica seus turnos; Codex relata tokens; Gemini não relata nada. Um turno que ninguém precificou é `$ –` com o motivo ao passar o mouse, nunca `$0.00`.
- **Um histórico que o Hark escreve.** Agentes ACP não guardam arquivo de sessão, então o Hark registra cada sessão que abre conforme os turnos acontecem: pesquisável, legível, retomável. O limite honesto: uma sessão aberta fora do Hark continua invisível.
- **O piso de produção.** `kubectl apply`, `terraform apply`, force push e companhia nunca são aprovados automaticamente, seja qual for o agente que os roda e como quer que ele chame sua ferramenta de shell.
- **Um teto.** O teto em dólares por turno é do Hark em todo agente.
- **Transferência.** "Troca esse chat pro Gemini" move a conversa: uma sessão nova no outro agente, aberta com um brief local de zero tokens, no mesmo cartão.

## Adicionar um agente que não vem embutido

```toml
[agents.myagent]
cmd  = "myagent"
args = ["--acp"]
```

Ele está no catálogo na próxima abertura. Se o CLI que a sua empresa permite não fala ACP, [abra uma issue](https://github.com/harkharness/hark/issues) descrevendo como ele transmite, se retoma sessões e se relata custo, ou [escreva o plugin](/docs/pt-br/build/plugin-contract/).

## Manter os agentes atualizados

Detectado não é atualizado. Configurações › Plugins compara a versão que o seu binário relata com a que o registro publica e mostra a linha de atualização. Um Gemini CLI antigo, por exemplo, responde a toda sessão nova com "This client is no longer supported"; isso é a atualização, não um problema de login.
