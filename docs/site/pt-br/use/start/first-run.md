---
title: Primeira execução
description: "O assistente de configuração: idioma, nome e atalho; um agente; o modelo de fala; o microfone."
sidebar:
  order: 3
banner:
  content: "<b>O Hark está em desenvolvimento.</b> A maior parte dos testes até agora foi feita com o CLI do Claude Code no macOS. Usa outro agente ou sistema? <a href=\"/docs/pt-br/build/testing-other-agents-and-oss/\">Ajude a testar.</a>"
source_hash: 8d4218edaf9e
---

Abra o `hark.app`. Um assistente de configuração roda uma vez e se recusa a deixar você com uma instalação pela metade:

1. **Idioma, nome e atalho.** O que o microfone espera ouvir, o que a tela mostra, como você quer chamar seu assistente e a tecla global que abre o microfone de qualquer lugar. Tudo editável depois em Configurações.
2. **Plugin de agente.** O Hark verifica quais CLIs de agente estão instalados e logados. O Claude Code se encaixa nativamente; Gemini CLI, Codex, Claude Code via ACP e qualquer outro agente Agent Client Protocol pelo plugin ACP. O catálogo mostra cada um, sua linha de instalação e se o seu build é o que o registro ACP publica. Nada roda até que pelo menos um seja detectado.
3. **Modelo de fala.** Baixa um modelo whisper (`small`, cerca de 466 MB, o padrão; `large-v3-turbo` se você quer mais precisão ao custo de latência). O download tem checksum e é retomável.
4. **Teste do microfone.** Grava dois segundos e mostra a transcrição, o que também dispara o pedido de permissão do macOS, com contexto, antes de você precisar dele no meio de algo.

Tudo o que o assistente escreve vai para `~/.hark/config.toml`, em texto puro, com seus comentários preservados nas edições seguintes.

## Dois idiomas, ao mesmo tempo

A gramática falada do Hark aceita português e inglês ao mesmo tempo; você nunca escolhe um. "abre a board" e "open the board" trocam de aba; "sim" e "yes" confirmam; "a primeira" e "the first one" escolhem o primeiro candidato. Palavras que significam coisas diferentes em cada idioma são desambiguadas em vez de descartadas: um "no" sozinho é uma recusa, "no projeto webhook" aponta um alvo.

Duas configurações separadas controlam o resto:

| Configuração | O que decide |
|---|---|
| `language` | o que o modelo de fala espera **ouvir**: um código fixo, ou `auto` para detectar a cada fala |
| `ui_language` | o que o Hark **escreve e fala**: interface, respostas locais, avisos e o idioma em que o agente é instruído a responder |

São separadas de propósito: muita gente fala português com uma interface em inglês, ou o contrário.

## Onde as coisas ficam

| Caminho | O quê |
|---|---|
| `~/.hark/config.toml` | sua configuração ([referência](/docs/pt-br/use/reference/configuration/)) |
| `~/.hark/index.db` | índice de sessões e livro-razão de gastos |
| `~/.hark/models/` | modelos de fala |
| `~/.hark/HARK.md` | a personalidade e os aprendizados do assistente, espelhados no arquivo de memória de cada agente ao lado |
| `~/.hark/artifacts/<space>/` | a árvore de documentação: páginas e o `index.json` que as ordena |
| `~/.hark/state.json` | quadro, workers, contexto ativo, o que cada agente mostrou oferecer |
| `~/.hark/sessions/<agent>/<id>.jsonl` | o registro próprio do Hark de sessões em agentes que não guardam nenhum |
| `<project>/.hark/` | diário, estado e briefs de despacho por projeto |

Nada disso sai da sua máquina.
