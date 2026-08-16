//! The pre-execution evaluator: a cheap model call that decides WHERE a
//! message should go before anything expensive or destructive runs.
//! Born from a real incident: a correction aimed at Vox was piped straight
//! into the wrong session and burned $18 on an inherited 1M-context model.

use serde::{Deserialize, Serialize};

/// What the evaluator can decide about a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateAction {
    /// Message is about Vox itself (correction, naming, focus): NEVER dispatch.
    MetaVox,
    /// Continue the focused task's session.
    ContinuarTask,
    /// The user means a DIFFERENT task than the focused one.
    TrocarTask,
    /// New piece of work, no matching task.
    NovaTask,
    /// Just a question for the vox ask path.
    Pergunta,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GateDecision {
    pub acao: GateAction,
    pub confianca: f64,
    pub motivo: String,
    #[serde(default)]
    pub aviso: Option<String>,
    #[serde(default)]
    pub task_alvo: Option<String>,
}

impl GateDecision {
    /// Anything uncertain or cost-flagged stops for a click first.
    pub fn needs_confirmation(&self) -> bool {
        self.confianca < 0.8 || self.aviso.as_deref().is_some_and(|a| !a.is_empty())
    }
}

/// Everything the evaluator sees besides the message itself.
#[derive(Debug, Clone, Default)]
pub struct GateContext {
    pub focused_task: Option<String>,
    pub focused_session: Option<String>,
    /// Real title of the focused session (may differ from the task name;
    /// a mismatch is exactly the incident we are guarding against).
    pub focused_session_title: Option<String>,
    pub board_lines: Vec<String>,
    pub live_workers: Vec<String>,
}

pub const GATE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "acao": {
      "type": "string",
      "enum": ["meta_vox", "continuar_task", "trocar_task", "nova_task", "pergunta"],
      "description": "meta_vox: a mensagem fala DO vox/da sessao/do foco (correcao, renomear, reclamacao) e nao deve executar nada. continuar_task: segue a task focada. trocar_task: o usuario quer outra task. nova_task: trabalho novo. pergunta: consulta informativa."
    },
    "confianca": { "type": "number", "description": "0 a 1" },
    "motivo": { "type": "string", "description": "Uma frase curta explicando a decisao" },
    "aviso": { "type": "string", "description": "OBRIGATORIO quando ha risco: titulo da sessao nao bate com a task, sessao usa modelo caro (opus/1m) ou historico muito grande. Vazio quando nao ha risco." },
    "task_alvo": { "type": "string", "description": "Titulo da task correta quando acao=trocar_task" }
  },
  "required": ["acao", "confianca", "motivo"]
}"#;

pub const GATE_SYSTEM_PROMPT: &str = "Voce e o roteador do Vox, um orquestrador de sessoes do \
Claude Code. Sua unica funcao e decidir PARA ONDE uma mensagem vai, nunca executa-la. \
Seja conservador: na duvida, confianca baixa. Mensagens que corrigem o Vox, reclamam de \
foco errado ou pedem para renomear/organizar sessoes sao SEMPRE meta_vox. \
Se o titulo real da sessao focada nao combina com a task focada, avise.";

/// Compact prompt for the evaluator (hundreds of tokens, haiku-priced).
pub fn build_prompt(message: &str, ctx: &GateContext) -> String {
    let mut sections = vec![format!("Mensagem do usuario:\n{message}")];
    if let Some(task) = &ctx.focused_task {
        sections.push(format!(
            "Task focada: {task}\nSessao focada: {} (titulo real: {})",
            ctx.focused_session.as_deref().unwrap_or("?"),
            ctx.focused_session_title.as_deref().unwrap_or("desconhecido"),
        ));
    } else {
        sections.push("Nenhuma task focada.".into());
    }
    if !ctx.board_lines.is_empty() {
        sections.push(format!("Tasks no quadro:\n{}", ctx.board_lines.join("\n")));
    }
    if !ctx.live_workers.is_empty() {
        sections.push(format!("Workers ativos: {}", ctx.live_workers.join(", ")));
    }
    sections.push("Decida a acao.".into());
    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> GateContext {
        GateContext {
            focused_task: Some("Doc playbooks de alertas - revisão comentários".into()),
            focused_session: Some("gladius-123".into()),
            focused_session_title: Some("Gladius: Hydrator e outras features".into()),
            board_lines: vec!["Doc playbooks de alertas [waiting] sessao=None".into()],
            live_workers: vec![],
        }
    }

    #[test]
    fn renders_prompt_with_message_and_context() {
        let prompt = build_prompt("vale renomear essa sessão", &ctx());
        assert!(prompt.contains("vale renomear essa sessão"));
        assert!(prompt.contains("Gladius: Hydrator"));
        assert!(prompt.contains("Doc playbooks"));
    }

    #[test]
    fn schema_is_valid_and_demands_the_essentials() {
        let schema: serde_json::Value = serde_json::from_str(GATE_SCHEMA).unwrap();
        let required = schema["required"].as_array().unwrap();
        for field in ["acao", "confianca", "motivo"] {
            assert!(required.iter().any(|v| v == field), "missing {field}");
        }
        let actions = schema["properties"]["acao"]["enum"].as_array().unwrap();
        assert!(actions.iter().any(|v| v == "meta_vox"));
        assert!(actions.iter().any(|v| v == "continuar_task"));
    }

    #[test]
    fn parses_decision_and_flags_what_needs_confirmation() {
        let decision: GateDecision = serde_json::from_str(
            r#"{"acao":"meta_vox","confianca":0.95,"motivo":"usuário corrige o foco do vox","aviso":null}"#,
        )
        .unwrap();
        assert_eq!(decision.acao, GateAction::MetaVox);
        assert!(!decision.needs_confirmation());

        let risky: GateDecision = serde_json::from_str(
            r#"{"acao":"continuar_task","confianca":0.55,"motivo":"ambíguo","aviso":"sessão usa opus-1m, turno caro"}"#,
        )
        .unwrap();
        assert!(risky.needs_confirmation(), "low confidence must confirm");

        let warned: GateDecision = serde_json::from_str(
            r#"{"acao":"continuar_task","confianca":0.9,"motivo":"ok","aviso":"contexto de 1M tokens"}"#,
        )
        .unwrap();
        assert!(warned.needs_confirmation(), "cost warning must confirm");

        let clean: GateDecision = serde_json::from_str(
            r#"{"acao":"continuar_task","confianca":0.9,"motivo":"segue a task focada"}"#,
        )
        .unwrap();
        assert!(!clean.needs_confirmation());
    }
}
