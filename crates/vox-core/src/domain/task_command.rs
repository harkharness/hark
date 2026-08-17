//! Pure parser for spoken board management: opening, renaming, pinning.
//! These never reach an LLM: they are local actions on the board.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskCommand {
    /// Read a thread; never executes anything.
    Open(String),
    /// Focus a task; optionally carry an instruction to run there.
    Switch {
        query: String,
        instruction: Option<String>,
    },
    Rename { query: String, title: String },
    Pin(String),
    Archive(String),
    /// Open a file in the local viewer (zero tokens).
    OpenFile {
        query: String,
        project: Option<String>,
    },
    /// Register a directory as a project.
    AddProject { path: String },
    /// Start a brand-new chat (fresh session) inside a project.
    NewChat { project: String },
}

/// Words that only glue the sentence together and never name a task.
const FILLER: &[&str] = &["a", "o", "as", "os", "da", "do", "das", "dos", "de", "task", "tarefa"];

fn clean_query(raw: &str) -> String {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let start = words
        .iter()
        .position(|w| !FILLER.contains(&w.to_lowercase().as_str()))
        .unwrap_or(words.len());
    words[start..].join(" ").trim().to_string()
}

/// Openers that mean "let me read that thread".
const OPEN_VERBS: &[&str] = &[
    "mostra o log", "mostra o histórico", "mostra o historico", "mostra a thread",
    "mostra o chat", "abre o log", "abre o histórico", "abre o historico",
    "abre a thread", "abre o chat", "ler a thread", "lê a thread",
];

const SWITCH_VERBS: &[&str] = &[
    "vai pra task", "vai para a task", "vai pra tarefa", "vai para a tarefa",
    "troca para a task", "troca pra task", "muda para a task", "muda pra task",
];
const RENAME_VERBS: &[&str] = &["renomeia", "renomear", "muda o titulo", "muda o título", "renomeie"];
const PIN_VERBS: &[&str] = &["fixa ", "fixar ", "prende "];
const ARCHIVE_VERBS: &[&str] = &["arquiva ", "arquivar "];
const OPEN_FILE_VERBS: &[&str] = &[
    "abre o arquivo", "abra o arquivo", "abrir o arquivo", "mostra o arquivo",
];
const ADD_PROJECT_VERBS: &[&str] = &[
    "adiciona o projeto", "adiciona projeto", "adicionar o projeto",
    "adiciona o diretório", "adiciona o diretorio", "registra o projeto",
];
const NEW_CHAT_VERBS: &[&str] = &[
    "novo chat", "nova sessão", "nova sessao", "nova task", "nova tarefa",
];

/// Parse a board command, or None when the sentence is real work.
pub fn parse(utterance: &str) -> Option<TaskCommand> {
    let lower = utterance.to_lowercase();
    let after = |needle: &str| {
        lower
            .find(needle)
            .map(|i| utterance[i + needle.len()..].trim().to_string())
    };

    if let Some(verb) = OPEN_FILE_VERBS.iter().find(|v| lower.contains(**v)) {
        let rest = after(verb)?;
        let rest_lower = rest.to_lowercase();
        let (query, project) = ["do projeto ", "no projeto "]
            .iter()
            .find_map(|sep| {
                rest_lower.find(sep).map(|i| {
                    (
                        rest[..i].trim().to_string(),
                        Some(rest[i + sep.len()..].trim().to_string()),
                    )
                })
            })
            .unwrap_or((rest.trim().to_string(), None));
        return (!query.is_empty()).then_some(TaskCommand::OpenFile {
            query,
            project: project.filter(|p| !p.is_empty()),
        });
    }
    if let Some(verb) = ADD_PROJECT_VERBS.iter().find(|v| lower.contains(**v)) {
        let path = crate::domain::project::path_from_speech(&after(verb)?);
        return (!path.is_empty()).then_some(TaskCommand::AddProject { path });
    }
    if lower.contains("projeto") {
        if let Some(verb) = NEW_CHAT_VERBS.iter().find(|v| lower.contains(**v)) {
            let rest = after(verb)?;
            let rest_lower = rest.to_lowercase();
            let project = rest_lower
                .find("projeto ")
                .map(|i| rest["projeto ".len() + i..].trim().to_string())?;
            return (!project.is_empty()).then_some(TaskCommand::NewChat { project });
        }
    }
    if let Some(verb) = SWITCH_VERBS.iter().find(|v| lower.contains(**v)) {
        let rest = after(verb)?;
        // "<query>" or "<query> e <instrução>"
        let (query, instruction) = match rest.to_lowercase().find(" e ") {
            Some(i) => (
                rest[..i].to_string(),
                Some(rest[i + " e ".len()..].trim().to_string()).filter(|s| !s.is_empty()),
            ),
            None => (rest, None),
        };
        let query = clean_query(&query);
        return (!query.is_empty()).then_some(TaskCommand::Switch { query, instruction });
    }
    if let Some(verb) = RENAME_VERBS.iter().find(|v| lower.contains(**v)) {
        let rest = after(verb)?;
        // "<query> para <novo titulo>"
        let (query, title) = rest.to_lowercase().rfind(" para ").map(|i| {
            (rest[..i].to_string(), rest[i + " para ".len()..].trim().to_string())
        })?;
        let query = clean_query(&query);
        return (!query.is_empty() && !title.is_empty())
            .then_some(TaskCommand::Rename { query, title });
    }
    for (verbs, build) in [
        (OPEN_VERBS, TaskCommand::Open as fn(String) -> TaskCommand),
        (PIN_VERBS, TaskCommand::Pin),
        (ARCHIVE_VERBS, TaskCommand::Archive),
    ] {
        if let Some(verb) = verbs.iter().find(|v| lower.contains(**v)) {
            let query = clean_query(&after(verb)?);
            return (!query.is_empty()).then(|| build(query));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_a_thread_for_reading() {
        assert_eq!(
            parse("mostra o log da migração do webhook"),
            Some(TaskCommand::Open("migração do webhook".into()))
        );
        assert_eq!(
            parse("abre o histórico de pagamentos"),
            Some(TaskCommand::Open("pagamentos".into()))
        );
        assert_eq!(
            parse("me mostra a thread dos alertas"),
            Some(TaskCommand::Open("alertas".into()))
        );
    }

    #[test]
    fn renames_a_task() {
        assert_eq!(
            parse("renomeia a task do webhook para Decom Carteira"),
            Some(TaskCommand::Rename {
                query: "webhook".into(),
                title: "Decom Carteira".into()
            })
        );
        assert_eq!(
            parse("muda o titulo da migração assinaturas para Assinaturas DNS"),
            Some(TaskCommand::Rename {
                query: "migração assinaturas".into(),
                title: "Assinaturas DNS".into()
            })
        );
    }

    #[test]
    fn pins_and_archives() {
        assert_eq!(
            parse("fixa a task de pagamentos"),
            Some(TaskCommand::Pin("pagamentos".into()))
        );
        assert_eq!(
            parse("arquiva a task dos alertas"),
            Some(TaskCommand::Archive("alertas".into()))
        );
    }

    #[test]
    fn switches_focus_with_optional_instruction() {
        assert_eq!(
            parse("vai pra task do webhook"),
            Some(TaskCommand::Switch {
                query: "webhook".into(),
                instruction: None
            })
        );
        assert_eq!(
            parse("vai para a task de pagamentos e roda os testes de novo"),
            Some(TaskCommand::Switch {
                query: "pagamentos".into(),
                instruction: Some("roda os testes de novo".into())
            })
        );
        assert_eq!(
            parse("troca para a task dos alertas"),
            Some(TaskCommand::Switch {
                query: "alertas".into(),
                instruction: None
            })
        );
    }

    #[test]
    fn opens_files_optionally_scoped_to_a_project() {
        assert_eq!(
            parse("abre o arquivo readme do projeto vox"),
            Some(TaskCommand::OpenFile {
                query: "readme".into(),
                project: Some("vox".into())
            })
        );
        assert_eq!(
            parse("mostra o arquivo main.rs"),
            Some(TaskCommand::OpenFile {
                query: "main.rs".into(),
                project: None
            })
        );
        assert_eq!(
            parse("abre o arquivo config.toml no projeto workspace-fabrica"),
            Some(TaskCommand::OpenFile {
                query: "config.toml".into(),
                project: Some("workspace-fabrica".into())
            })
        );
    }

    #[test]
    fn adds_projects_from_typed_or_spoken_paths() {
        assert_eq!(
            parse("adiciona o projeto ~/Projects/vox"),
            Some(TaskCommand::AddProject { path: "~/Projects/vox".into() })
        );
        assert_eq!(
            parse("adiciona o diretório home projects demo"),
            Some(TaskCommand::AddProject { path: "~/projects/demo".into() })
        );
    }

    #[test]
    fn starts_new_chats_inside_a_project() {
        assert_eq!(
            parse("novo chat no projeto vox"),
            Some(TaskCommand::NewChat { project: "vox".into() })
        );
        assert_eq!(
            parse("nova sessão no projeto workspace-fabrica"),
            Some(TaskCommand::NewChat { project: "workspace-fabrica".into() })
        );
        assert_eq!(
            parse("nova task no projeto demo"),
            Some(TaskCommand::NewChat { project: "demo".into() })
        );
    }

    #[test]
    fn leaves_real_work_alone() {
        // Anything that is not board bookkeeping must fall through.
        assert_eq!(parse("continua a migração do webhook"), None);
        assert_eq!(parse("quais as pendências de hoje?"), None);
        assert_eq!(parse("abre o PR do DNS antigo"), None);
        assert_eq!(parse("adiciona logs no serviço de webhook"), None);
        assert_eq!(parse("cria uma nova rota no gateway"), None);
    }
}
