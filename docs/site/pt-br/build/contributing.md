---
title: Contribuir
description: Boas primeiras contribuições, a preparação, as verificações e como o código é escrito aqui.
sidebar:
  order: 1
source_hash: 01b84c61567e
---

Obrigado por ajudar. O Hark tem uma forma clara, uma cabine sobre os CLIs de agente que as pessoas já pagam, local-first, honesto sobre custo, e as melhores contribuições mantêm essa forma. Para qualquer coisa maior que uma correção, abra uma issue antes, para combinarmos a forma antes de você gastar um fim de semana nisso.

## Boas primeiras contribuições

- **Um agente que fala ACP.** A maioria dos agentes novos é uma entrada em `builtins()` (`crates/hark-core/src/domain/agents.rs`), com seu `cmd`, `args`, arquivo de memória e camadas de modelo, mais uma linha na documentação.
- **Um backend para um CLI que não fala ACP.** Comece pelo [contrato de plugin](/docs/pt-br/build/plugin-contract/).
- **Testar o Hark em outro agente ou sistema.** A coisa mais valiosa agora: veja [Testar outros agentes e sistemas](/docs/pt-br/build/testing-other-agents-and-oss/).
- **Traduções**, do app e desta documentação: veja [Traduções](/docs/pt-br/build/translations/).
- **Frases de voz.** `crates/hark-core/tests/corpus/voice_corpus.jsonl` mantém o classificador de voz honesto: uma frase, o catálogo contra o qual foi dita, e onde ela deve cair.
- **Relatos de bug** com a versão do Hark, a versão e a arquitetura do sistema, e o CLI de agente com sua versão. O "Reportar um problema" do app preenche isso por você.

## Preparação

Você precisa de macOS 12+, as ferramentas de linha de comando do Xcode, Rust (stable; a CI fixa 1.98.0), Node 22 e npm, e pelo menos um CLI de agente logado.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev
```

A primeira execução baixa o modelo de fala (cerca de 500 MB) e pede o microfone. Para olhar a janela sem backend, e para tirar capturas de tela: `npm run dev`, depois `http://localhost:1420/?demo=home`; veja [Frontend](/docs/pt-br/build/frontend/).

## Verificações

Tudo o que a CI roda, rodável localmente:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npx tsc --noEmit
npm test
npm run build
npm run docs:check
```

## Como o código é escrito aqui

- **Núcleo funcional, casca imperativa.** `crates/hark-core/src/domain` é puro: sem I/O, tipos imutáveis, testado sem microfone, banco de dados ou rede. Adaptadores e a camada Tauri ficam finos.
- **Testes primeiro no domínio.** Escreva o teste, veja-o falhar pelo motivo que você espera, e só então faça passar.
- **A saída do agente não é confiável.** Um repositório pode injetar um prompt e fazer um agente imprimir qualquer coisa. Código que transforma texto do agente em ação (abrir um arquivo, escrever em disco, o terminal, iniciar um processo) passa por uma guarda com teste. `SECURITY.md` tem o modelo.
- **Dados de exemplo são inventados.** Fixtures, mocks, capturas de tela e exemplos nunca carregam nomes reais de empresa, cliente, ticket ou pessoa.
- **Commits** são em inglês, `tipo(escopo): resumo` em minúsculas (`fix`, `feat`, `chore`, `ci`, `docs`), com um corpo que explica o porquê.

## Licença

Ao contribuir, você concorda que sua contribuição é licenciada sob MIT ou Apache-2.0, como o resto do Hark, sem termos ou condições adicionais.
