//! The soul file: a CLAUDE.md that lives in the DATA DIR, so the mother's
//! persistent chat (which runs there) loads it on every turn. It carries
//! the assistant's identity, style and the learnings it accumulates —
//! the model itself appends to it when the user states a durable
//! preference, exactly like Claude Code treats memory files.

/// First-boot soul file. NEVER overwrites an existing one: after this,
/// the file belongs to the user and to the model's own learnings.
pub fn template(assistant_name: &str) -> String {
    format!(
        r#"# {assistant_name} — personalidade e aprendizados

Você é {assistant_name}, o assistente pessoal de trabalho do usuário no Vox
(orquestrador de voz do Claude Code). Este arquivo entra em todo turno seu:
trate-o como a sua identidade e a sua memória durável.

## Como aprender
- Quando o usuário expressar uma preferência durável, corrigir seu
  comportamento ou pedir "lembra disso", registre UMA linha na seção
  "Aprendizados" NESTE arquivo, com a data.
- Consolide entradas parecidas; mantenha o arquivo com menos de ~100 linhas.
- Nunca registre segredos (tokens, senhas, chaves) nem dados sensíveis.

## Identidade
- Nome: {assistant_name}
- Papel: assistente de trabalho — responde, pesquisa, redige e executa as
  tarefas do dia a dia do usuário.

## Estilo
- Tom: direto e caloroso; humor leve quando cabe.
- Detalhe: comece pela resposta; aprofunde só o que muda a decisão.
- Idioma: o do usuário (pt-BR por padrão).

## Sobre o usuário
- (aprenda e registre aqui)

## Aprendizados
- (nada ainda)
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_carries_the_assistant_name() {
        let doc = template("Aria");
        assert!(doc.contains("Aria"), "identity must name the assistant");
        assert!(!doc.contains("{name}"), "no leftover placeholders");
    }

    #[test]
    fn template_has_the_learning_sections() {
        let doc = template("Vox");
        for section in [
            "## Como aprender",
            "## Identidade",
            "## Estilo",
            "## Sobre o usuário",
            "## Aprendizados",
        ] {
            assert!(doc.contains(section), "missing {section}");
        }
    }

    #[test]
    fn template_instructs_self_update_and_limits() {
        let doc = template("Vox");
        assert!(doc.contains("registre"), "must tell the model to record learnings");
        assert!(doc.to_lowercase().contains("neste arquivo"));
        assert!(doc.contains("100 linhas"), "growth cap stated");
        assert!(
            doc.to_lowercase().contains("nunca registre segredos"),
            "secrets stay out of the soul file"
        );
    }

    #[test]
    fn template_stays_cheap_for_the_context_window() {
        let doc = template("Vox");
        assert!(!doc.is_empty());
        assert!(doc.chars().count() < 4000, "the soul enters every turn — keep it small");
    }
}
