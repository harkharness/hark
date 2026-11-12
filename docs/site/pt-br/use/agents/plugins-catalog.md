---
title: Plugins e troca de agentes
description: O catálogo em Configurações, qual agente abre chats novos, e mover uma conversa entre agentes.
sidebar:
  order: 3
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 4b108c388809
---

Configurações › **Plugins** é o catálogo: todo agente que o Hark conhece, se o binário está nesta máquina e a linha de instalação quando não está, o que cada plugin consegue fazer (retomar, permissões, relato de custo, histórico, comandos de barra…), como o agente chama cada camada de modelo, qual build você tem contra o que o registro ACP publica, e qual deles abre chats novos.

## Uma cabine, vários agentes

Todo chat pertence ao agente que o abriu, e as pílulas ao lado do compositor (modo de permissão, camada de modelo, esforço) falam com esse agente no vocabulário dele. Um ajuste que o agente não oferece fica na tela, desabilitado, com o motivo: escondido se leria como "o Hark não consegue".

Dois comandos falados movem entre agentes:

- *"abre um chat no gemini em webhook"*: um chat novo nesse projeto, nesse agente.
- *"troca esse chat pro gemini"*: a conversa em que você está se move. Uma sessão nova no outro agente abre com um brief local, de zero tokens, da conversa até ali, no mesmo cartão. O popover ⓘ de uma conversa ao vivo tem um botão por agente disponível para o mesmo movimento.

## Conectores

A seção *Conectores* da barra de sessão lista os servidores MCP que a sessão abriu, com seu estado. Reconectar fica em Configurações › Plugins.
