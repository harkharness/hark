//! The soul file: a CLAUDE.md that lives in the DATA DIR, so the mother's
//! persistent chat (which runs there) loads it on every turn. It carries
//! the assistant's identity, style and the learnings it accumulates —
//! the model itself appends to it when the user states a durable
//! preference, exactly like Claude Code treats memory files.

/// First-boot soul file. NEVER overwrites an existing one: after this,
/// the file belongs to the user and to the model's own learnings.
pub fn template(assistant_name: &str, lang: crate::domain::lang::Lang) -> String {
    // The seed prose stays Portuguese — the model reads both — but the
    // language DIRECTIVE has to agree with the interface, or the mother
    // chat answers in one language while the screen speaks the other.
    let language = lang.pick("o do usuário (português brasileiro)", "English");
    format!(
        r#"# {assistant_name} — personalidade e aprendizados

Você é {assistant_name}, o assistente pessoal de trabalho do usuário no Hark
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
- Idioma: {language}.

## Sobre o usuário
- (aprenda e registre aqui)

## Aprendizados
- (nada ainda)
"#
    )
}

/// The identity the CHEAP ask carries: only "## Identidade" and
/// "## Estilo" from the soul file, hard-clipped — a few hundred chars of
/// tone, never the learnings (those belong to the full chat).
pub fn excerpt(doc: &str) -> String {
    let mut out = String::new();
    let mut keep = false;
    for line in doc.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            keep = matches!(title.trim(), "Identidade" | "Estilo");
            continue;
        }
        if keep && !line.trim().is_empty() {
            out.push_str(line);
            out.push('\n');
        }
    }
    let out = out.trim_end().to_string();
    if out.chars().count() > 600 {
        let mut clipped: String = out.chars().take(600).collect();
        clipped.push('…');
        clipped
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::lang::Lang;

    #[test]
    fn excerpt_takes_identity_and_style_only() {
        let doc = template("Aria", Lang::Pt);
        let ex = excerpt(&doc);
        assert!(ex.contains("Aria"), "identity survives");
        assert!(ex.contains("Tom:"), "style survives");
        assert!(!ex.contains("Como aprender"), "meta-instructions stay out");
        assert!(!ex.contains("Aprendizados"), "learnings stay out of the ask");
    }

    #[test]
    fn excerpt_stays_tiny_and_handles_junk() {
        let long = format!("## Identidade\n{}\n## Estilo\n- Tom: x\n", "linha grande\n".repeat(200));
        assert!(excerpt(&long).chars().count() <= 620, "hard clip");
        assert_eq!(excerpt(""), "");
        assert_eq!(excerpt("# sem seções\ntexto"), "");
    }

    #[test]
    fn template_carries_the_assistant_name() {
        let doc = template("Aria", Lang::Pt);
        assert!(doc.contains("Aria"), "identity must name the assistant");
        assert!(!doc.contains("{name}"), "no leftover placeholders");
    }

    #[test]
    fn template_has_the_learning_sections() {
        let doc = template("Hark", Lang::Pt);
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
        let doc = template("Hark", Lang::Pt);
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
        let doc = template("Hark", Lang::Pt);
        assert!(!doc.is_empty());
        assert!(doc.chars().count() < 4000, "the soul enters every turn — keep it small");
    }
}
