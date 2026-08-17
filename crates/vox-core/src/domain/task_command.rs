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
    /// Start a brand-new chat (fresh session) inside a project, optionally
    /// carrying its first instruction.
    NewChat {
        project: String,
        instruction: Option<String>,
    },
    /// Open a project's dedicated window; optionally dispatch work there.
    OpenProject {
        query: String,
        instruction: Option<String>,
    },
    /// Open the global HQ window (board/costs across every project).
    OpenHq { tab: String },
    /// Recover an EXISTING Claude Code session by topic. The local index
    /// already knows every session on this machine, so this must never
    /// cost a token — and never spawn an agent to go digging.
    FindSession { query: String },
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
    // Recovery of finished/hidden tasks: focusing one pulls it back to work.
    "retoma a task", "retomar a task", "retoma a tarefa", "retomar a tarefa",
    "reabre a task", "reabrir a task", "volta pra task", "volta para a task",
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
/// Unambiguous: these words always mean "open a fresh Claude session".
const NEW_CHAT_VERBS: &[&str] = &[
    "novo chat", "inicia um chat", "iniciar um chat", "roda um chat",
    "rode um chat", "executa um chat", "cria um chat", "começa um chat",
    "comeca um chat",
];
/// Ambiguous ("nova task" is usually real work): only a chat command when
/// the sentence names a project explicitly.
const NEW_CHAT_GATED_VERBS: &[&str] = &[
    "nova sessão", "nova sessao", "nova task", "nova tarefa",
];
const OPEN_PROJECT_VERBS: &[&str] = &[
    "abre o projeto", "abra o projeto", "abrir o projeto",
    "abre a janela do projeto", "abra a janela do projeto",
];
/// (verbs, tab) pairs for the global HQ window.
const HQ_VERBS: &[(&str, &str)] = &[
    ("abre a board", "board"), ("abra a board", "board"), ("abre o board", "board"),
    ("mostra a board", "board"), ("abre o quadro", "board"), ("mostra o quadro", "board"),
    ("abre os custos", "custos"), ("abra os custos", "custos"),
    ("mostra os custos", "custos"), ("abre custos", "custos"),
];

/// Recovering a session someone remembers having. The word "sessão"/"chat"
/// is mandatory: without it the sentence is work, not a lookup.
const FIND_SESSION_VERBS: &[&str] = &[
    "tinha um chat", "tinha uma sessão", "tinha uma sessao",
    "recupera a sessão", "recupera a sessao", "recupera o chat",
    "recuperar a sessão", "recuperar a sessao", "recuperar o chat",
    "acha a sessão", "acha a sessao", "acha o chat",
    "encontra a sessão", "encontra a sessao", "encontra o chat",
    "procura a sessão", "procura a sessao", "procura o chat",
    "retoma a sessão", "retoma a sessao", "retoma o chat",
    "retomar a sessão", "retomar a sessao", "retomar o chat",
    "continua a sessão", "continua a sessao", "continua o chat",
    "volta pra sessão", "volta para a sessão", "volta pro chat",
    "abre a sessão", "abre a sessao", "abrir a sessão",
];

/// Leading words that only point at the topic ("aberto de", "sobre a").
const TOPIC_FILLER: &[&str] = &[
    "aberto", "aberta", "sobre", "que", "com", "falando", "focado", "focada",
    "tratando", "chat", "sessão", "sessao", "a", "o", "as", "os", "da", "do",
    "das", "dos", "de", "em", "no", "na", "para", "pra",
];

/// Drop the words before the topic itself, keeping everything after.
fn clean_topic(raw: &str) -> String {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let start = words
        .iter()
        .position(|w| !TOPIC_FILLER.contains(&w.to_lowercase().as_str()))
        .unwrap_or(words.len());
    words[start..].join(" ").trim().to_string()
}

/// Split "<head> <sep> <tail>" on the first separator, both halves trimmed.
fn split_once_word(text: &str, sep: &str) -> Option<(String, String)> {
    text.to_lowercase().find(sep).map(|i| {
        (
            text[..i].trim().to_string(),
            text[i + sep.len()..].trim().to_string(),
        )
    })
}

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
    let chat_verb = NEW_CHAT_VERBS
        .iter()
        .find(|v| lower.contains(**v))
        .or_else(|| {
            NEW_CHAT_GATED_VERBS
                .iter()
                .find(|v| lower.contains(**v))
                .filter(|_| lower.contains("projeto"))
        });
    if let Some(verb) = chat_verb {
        let rest = after(verb)?;
        // "... para <instrução>" carries the first task of the chat.
        let (place, instruction) = split_once_word(&rest, " para ")
            .map(|(p, i)| (p, Some(i).filter(|s| !s.is_empty())))
            .unwrap_or((rest.trim().to_string(), None));
        // "no projeto X" | "no X" | "em X" — the project name is what's left.
        let project = ["no projeto ", "na projeto ", "no ", "na ", "em "]
            .iter()
            .find_map(|sep| split_once_word(&place, sep).map(|(_, tail)| tail))
            .unwrap_or(place);
        return (!project.is_empty()).then_some(TaskCommand::NewChat { project, instruction });
    }
    if let Some(verb) = OPEN_PROJECT_VERBS.iter().find(|v| lower.contains(**v)) {
        let rest = after(verb)?;
        // "<projeto>" or "<projeto> e <instrução>": open AND dispatch.
        let (query, instruction) = split_once_word(&rest, " e ")
            .map(|(q, i)| (q, Some(i).filter(|s| !s.is_empty())))
            .unwrap_or((rest.trim().to_string(), None));
        return (!query.is_empty()).then_some(TaskCommand::OpenProject { query, instruction });
    }
    if let Some((_, tab)) = HQ_VERBS.iter().find(|(v, _)| lower.contains(*v)) {
        return Some(TaskCommand::OpenHq { tab: (*tab).to_string() });
    }
    if let Some(verb) = FIND_SESSION_VERBS.iter().find(|v| lower.contains(**v)) {
        let query = clean_topic(&after(verb)?);
        return (!query.is_empty()).then_some(TaskCommand::FindSession { query });
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
    fn recovers_an_existing_session_by_topic() {
        // Spoken the way a person actually says it, mid-sentence.
        assert_eq!(
            parse("tinha um chat aberto de busca de alertas e cluster"),
            Some(TaskCommand::FindSession {
                query: "busca de alertas e cluster".into()
            })
        );
        assert_eq!(
            parse("recupera a sessão dos alertas"),
            Some(TaskCommand::FindSession {
                query: "alertas".into()
            })
        );
        assert_eq!(
            parse("acha o chat sobre a migração do webhook"),
            Some(TaskCommand::FindSession {
                query: "migração do webhook".into()
            })
        );
        assert_eq!(
            parse("retoma a sessão de pagamentos"),
            Some(TaskCommand::FindSession {
                query: "pagamentos".into()
            })
        );
        // Dictated whole, filler and all — the sentence that used to cost a
        // full worker digging through log files.
        assert_eq!(
            parse("eu estou dizendo que tinha um chat focado na busca de alertas e do cluster"),
            Some(TaskCommand::FindSession {
                query: "busca de alertas e do cluster".into()
            })
        );
    }

    #[test]
    fn session_recovery_never_swallows_real_work() {
        // No session/chat word: this is work, and work belongs to a worker.
        assert_eq!(parse("continua a migração de assinaturas"), None);
        assert_eq!(parse("recupera o backup do banco"), None);
        // Board tasks keep their own verb: a task is not a session.
        assert_eq!(
            parse("retoma a task do webhook"),
            Some(TaskCommand::Switch {
                query: "webhook".into(),
                instruction: None
            })
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
    fn recovers_finished_tasks_by_resume_verbs() {
        assert_eq!(
            parse("retoma a task do hydrator"),
            Some(TaskCommand::Switch {
                query: "hydrator".into(),
                instruction: None
            })
        );
        assert_eq!(
            parse("quero retomar a task do hydrator"),
            Some(TaskCommand::Switch {
                query: "hydrator".into(),
                instruction: None
            })
        );
        assert_eq!(
            parse("reabre a task dos alertas e continua a revisão"),
            Some(TaskCommand::Switch {
                query: "alertas".into(),
                instruction: Some("continua a revisão".into())
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
            Some(TaskCommand::NewChat { project: "vox".into(), instruction: None })
        );
        assert_eq!(
            parse("nova sessão no projeto workspace-fabrica"),
            Some(TaskCommand::NewChat {
                project: "workspace-fabrica".into(),
                instruction: None
            })
        );
        assert_eq!(
            parse("nova task no projeto demo"),
            Some(TaskCommand::NewChat { project: "demo".into(), instruction: None })
        );
    }

    #[test]
    fn new_chats_accept_bare_project_names_and_instructions() {
        // "projeto" is optional; STT rarely says it.
        assert_eq!(
            parse("inicia um chat no workspace-fabrica"),
            Some(TaskCommand::NewChat {
                project: "workspace-fabrica".into(),
                instruction: None
            })
        );
        // "... para <instrução>" carries the first task of the chat.
        assert_eq!(
            parse("roda um chat no workspace fábrica para reindexar as tasks perdidas"),
            Some(TaskCommand::NewChat {
                project: "workspace fábrica".into(),
                instruction: Some("reindexar as tasks perdidas".into())
            })
        );
        assert_eq!(
            parse("novo chat no projeto vox para revisar o README"),
            Some(TaskCommand::NewChat {
                project: "vox".into(),
                instruction: Some("revisar o README".into())
            })
        );
    }

    #[test]
    fn opens_project_windows_with_optional_instruction() {
        assert_eq!(
            parse("abra o projeto vox"),
            Some(TaskCommand::OpenProject { query: "vox".into(), instruction: None })
        );
        // Works inside a longer sentence (STT never starts at the verb).
        assert_eq!(
            parse("eu quero que você abra o projeto workspace fábrica"),
            Some(TaskCommand::OpenProject {
                query: "workspace fábrica".into(),
                instruction: None
            })
        );
        // "... e <instrução>" opens AND dispatches inside it.
        assert_eq!(
            parse("abre o projeto workspace fábrica e roda a reindexação das tasks"),
            Some(TaskCommand::OpenProject {
                query: "workspace fábrica".into(),
                instruction: Some("roda a reindexação das tasks".into())
            })
        );
        // File commands keep priority even when they cite a project.
        assert_eq!(
            parse("abre o arquivo readme do projeto vox"),
            Some(TaskCommand::OpenFile {
                query: "readme".into(),
                project: Some("vox".into())
            })
        );
    }

    #[test]
    fn opens_the_global_board_and_costs() {
        assert_eq!(parse("abre a board"), Some(TaskCommand::OpenHq { tab: "board".into() }));
        assert_eq!(parse("mostra o quadro"), Some(TaskCommand::OpenHq { tab: "board".into() }));
        assert_eq!(parse("abre os custos"), Some(TaskCommand::OpenHq { tab: "custos".into() }));
        assert_eq!(
            parse("mostra os custos"),
            Some(TaskCommand::OpenHq { tab: "custos".into() })
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
        // "nova task" without a named project is real work for the gate.
        assert_eq!(parse("nova task adiciona logs no serviço"), None);
    }
}
