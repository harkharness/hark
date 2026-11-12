---
title: Testar outros agentes e sistemas
description: A maior parte do Hark foi testada com o Claude Code no macOS. Veja como ajudar com o resto.
sidebar:
  order: 5
source_hash: 4d797b816b55
---

A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS, Apple Silicon e Intel. Gemini CLI, Codex e os outros agentes ACP rodam, e são publicados, mas viram bem menos horas de uso. Linux e Windows são as próximas releases e ainda não são usáveis. É aqui que ajuda importa mais.

## Testar outro agente

1. Instale o CLI do agente e faça login com ele no seu terminal primeiro.
2. Abra Configurações › Plugins: o cartão mostra se o Hark o detectou, a versão contra o registro, e o que o plugin diz que consegue fazer.
3. Abra um chat nele ("abre um chat no gemini em webhook"), envie alguns turnos, dispare uma permissão, deixe um turno rodar um comando de shell e olhe o livro-razão.
4. Reporte o que não bateu com a documentação: [abra uma issue](https://github.com/harkharness/hark/issues) com o agente e sua versão, seu sistema e arquitetura, a release do Hark e os diagnósticos do app ("Reportar um problema" os anexa, limpos).

O que mais queremos saber: as permissões chegaram como cartões, as pílulas de modelo e esforço falaram com o agente corretamente, o livro-razão mostrou tokens e, onde o agente relata, dólares, e um chat retomado retomou de verdade.

## Testar outro sistema

Os ports são acompanhados no repositório. O que se sabe que é só macOS hoje: texto em fala (`say`), os sons de aviso, o build Metal do modelo de fala, abrir arquivos no Finder, a barra de título sobreposta da janela, os atalhos (`Cmd`) e o instalador. Se você consegue compilar do código-fonte no Linux ou no Windows, o relato mais útil é a primeira coisa que quebra, com o log do build.

## Reportar

Todo relato é público; leia o que os diagnósticos contêm antes de enviar. Nada sai da sua máquina pelo próprio Hark.
