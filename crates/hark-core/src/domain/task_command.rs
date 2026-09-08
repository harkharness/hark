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
    /// Move a task to done. `None` means the one you are in — closing the
    /// task you are looking at is the common case, and naming it again is
    /// exactly the ceremony that made people type it at the agent instead.
    Done(Option<String>),
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
    /// Compact the focused session's context (delivered as "/compact").
    Compact,
    /// Switch the focused worker's permission mode (CLI flag value).
    SetMode { mode: String },
    /// Open the settings screen (config.toml behind a UI).
    OpenSettings,
}

/// Words that only glue the sentence together and never name a task.
const FILLER: &[&str] = &[
    "a", "o", "as", "os", "da", "do", "das", "dos", "de", "task", "tarefa",
    "the", "of", "for", "to", "on",
];

fn clean_query(raw: &str) -> String {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let start = words
        .iter()
        .position(|w| !FILLER.contains(&w.to_lowercase().as_str()))
        .unwrap_or(words.len());
    words[start..].join(" ").trim().to_string()
}

/// Openers that mean "let me read that thread" (the log viewer).
const OPEN_VERBS: &[&str] = &[
    "mostra o log", "mostra o histórico", "mostra o historico", "mostra a thread",
    "mostra o chat", "abre o log", "abre o histórico", "abre o historico",
    "ler a thread", "lê a thread",
    "show the log", "show the history", "show the thread", "open the log",
    "open the history", "read the thread",
];

const SWITCH_VERBS: &[&str] = &[
    "vai pra task", "vai para a task", "vai pra tarefa", "vai para a tarefa",
    "troca para a task", "troca pra task", "muda para a task", "muda pra task",
    "go to task", "go to the task", "switch to task", "switch to the task",
    "move to the task", "back to the task", "resume the task",
    // Recovery of finished/hidden tasks: focusing one pulls it back to work.
    "retoma a task", "retomar a task", "retoma a tarefa", "retomar a tarefa",
    "reabre a task", "reabrir a task", "volta pra task", "volta para a task",
    // "abre o chat de X" = go back to WORK on X, not the read-only log.
    "abre o chat", "abre a thread", "abra o chat", "abrir o chat",
];
const RENAME_VERBS: &[&str] =
    &["renomeia", "renomear", "muda o titulo", "muda o título", "renomeie", "rename"];
const PIN_VERBS: &[&str] = &["fixa ", "fixar ", "prende ", "pin "];
const ARCHIVE_VERBS: &[&str] = &["arquiva ", "arquivar ", "archive "];
/// Closing a task. The VERB alone is never enough — "fecha o PR" and
/// "conclui a migração" are jobs, not board moves — so a match also needs
/// the word task/tarefa/chat as the thing being closed.
const DONE_VERBS: &[&str] = &[
    "fecha", "fechar", "fechamos", "conclui", "concluir", "concluímos", "concluimos",
    "concluída", "concluida", "concluído", "concluido", "termina", "terminar",
    "terminamos", "encerra", "encerrar", "close", "closes", "finish", "done", "mark", "finished",
];
const DONE_OBJECTS: &[&str] = &["task", "tarefa", "chat"];
const OPEN_FILE_VERBS: &[&str] = &[
    "abre o arquivo", "abra o arquivo", "abrir o arquivo", "mostra o arquivo",
    "open the file", "show the file", "open file",
];
const ADD_PROJECT_VERBS: &[&str] = &[
    "adiciona o projeto", "adiciona projeto", "adicionar o projeto",
    "adiciona o diretório", "adiciona o diretorio", "registra o projeto",
    // "um novo/cria um projeto EM <path>" registers too — "abre O projeto"
    // (registered ones) stays OPEN_PROJECT.
    "novo projeto", "cria um projeto", "criar um projeto", "cria projeto",
    "abre um projeto", "abra um projeto", "abrir um projeto",
    "inicia um projeto", "começa um projeto", "comeca um projeto",
];

/// Location glue after a new-project verb ("em", "no", "dentro de").
fn clean_path_lead(raw: &str) -> &str {
    ["em ", "no ", "na ", "dentro de ", "dentro do "]
        .iter()
        .find_map(|lead| raw.strip_prefix(lead))
        .unwrap_or(raw)
        .trim()
}
/// Unambiguous: these words always mean "open a fresh Claude session".
const NEW_CHAT_VERBS: &[&str] = &[
    "novo chat", "inicia um chat", "iniciar um chat", "roda um chat",
    "rode um chat", "executa um chat", "cria um chat", "começa um chat",
    "comeca um chat",
    "new chat", "start a chat", "open a new chat", "create a chat",
];
/// Ambiguous ("nova task" is usually real work): only a chat command when
/// the sentence names a project explicitly.
const NEW_CHAT_GATED_VERBS: &[&str] = &[
    "nova sessão", "nova sessao", "nova task", "nova tarefa",
    "new session", "new task",
];
const OPEN_PROJECT_VERBS: &[&str] = &[
    "abre o projeto", "abra o projeto", "abrir o projeto",
    "abre a janela do projeto", "abra a janela do projeto",
    "open the project window", "open the project", "open project",
];
/// (verbs, tab) pairs for the global HQ window.
const HQ_VERBS: &[(&str, &str)] = &[
    ("abre a board", "board"), ("abra a board", "board"), ("abre o board", "board"),
    ("mostra a board", "board"), ("abre o quadro", "board"), ("mostra o quadro", "board"),
    ("abre os custos", "custos"), ("abra os custos", "custos"),
    ("mostra os custos", "custos"), ("abre custos", "custos"),
    // English. The tab ids stay as they are — they are internal names.
    ("open the board", "board"), ("show the board", "board"),
    ("open board", "board"),
    ("open the costs", "custos"), ("show the costs", "custos"),
    ("open costs", "custos"), ("show me the spend", "custos"),
];

/// Words that point at the focused thing instead of naming another one
/// ("renomeia ESSE CHAT para…"). Stripping them all leaves an empty query,
/// which the shell resolves to whatever is focused.
const SELF_WORDS: &[&str] = &[
    "essa", "esse", "esta", "este", "dessa", "desse", "desta", "deste",
    "atual", "chat", "sessão", "sessao", "conversa", "chamada", "aqui",
];

fn clean_rename_target(raw: &str) -> String {
    raw.split_whitespace()
        .filter(|w| {
            let w = w.to_lowercase();
            !SELF_WORDS.contains(&w.as_str()) && !FILLER.contains(&w.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

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
    // "busca" was missing while its three synonyms were present — same
    // verb, same intent, and the sentence fell through to a prose answer.
    "busca a sessão", "busca a sessao", "busca o chat", "buscar o chat",
    "busca no histórico", "busca no historico", "buscar no histórico",
    "procura no histórico", "procura no historico", "acha no histórico",
    "acha no historico", "encontra no histórico", "encontra no historico",
    "there was a chat", "recover the chat", "recover the session",
    "find the chat", "find the session", "look for the session",
    "resume the session", "back to the session", "open the session",
];

/// Compaction of the FOCUSED chat's context. The object words (contexto/
/// sessão/chat) are mandatory: "compactar os arquivos" is real work.
const COMPACT_VERBS: &[&str] = &[
    "compacta o contexto", "compacta a sessão", "compacta a sessao",
    "compacta o chat", "compacta a conversa", "compacta esse chat",
    "compactação do contexto", "compactacao do contexto",
    "faz a compactação", "faz a compactacao",
    "compact the context", "compact the session", "compact the chat",
    "compact this chat",
];

/// Mid-session permission-mode switch ("muda o modo pra automático").
/// A bare mode word without one of these verbs is a SPAWN directive
/// (directives.rs), never a command.
const SET_MODE_VERBS: &[&str] = &[
    "muda o modo", "troca o modo", "coloca no modo", "muda pro modo",
    "troca pro modo", "altera o modo", "modo de permissão", "modo de permissao",
    "change the mode", "switch the mode", "set the mode", "permission mode",
];

/// (needle in the words after the verb, CLI --permission-mode value).
/// Bypass only via words that say "permission" out loud.
const SET_MODE_TARGETS: &[(&str, &str)] = &[
    ("ignora", "bypassPermissions"),
    ("bypass", "bypassPermissions"),
    ("sem trava", "bypassPermissions"),
    ("edi", "acceptEdits"),
    ("plan", "plan"),
    ("manual", "manual"),
    ("auto", "auto"),
];

/// Leading words that only point at the topic ("aberto de", "sobre a").
const TOPIC_FILLER: &[&str] = &[
    "aberto", "aberta", "sobre", "que", "com", "falando", "focado", "focada",
    "tratando", "chat", "sessão", "sessao", "a", "o", "as", "os", "da", "do",
    "das", "dos", "de", "em", "no", "na", "para", "pra",
];

/// Drop the words before the topic itself, keeping everything after.
fn clean_topic(raw: &str) -> String {
    // A spoken lookup ends with what to DO with the hit: "…last version e
    // abra ele". That tail is punctuation, not part of the session's name.
    let lower = raw.to_lowercase();
    let head = ["  e abre", " e abre", " e abra", " e abrir", " e mostra", " e vai"]
        .iter()
        .filter_map(|tail| lower.find(tail))
        .min()
        .map_or(raw, |cut| &raw[..cut]);

    let words: Vec<&str> = head.split_whitespace().collect();
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
        let rest = after(verb)?;
        let path = crate::domain::project::path_from_speech(clean_path_lead(&rest));
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
            .or_else(|| split_once_word(&rest, " to "))
            .map(|(p, i)| (p, Some(i).filter(|s| !s.is_empty())))
            .unwrap_or((rest.trim().to_string(), None));
        // "no projeto X" | "no X" | "em X" | "in X" — the name is the rest.
        let project = [
            "no projeto ", "na projeto ", "no ", "na ", "em ",
            "in the project ", "in project ", "in ", "on ",
        ]
            .iter()
            .find_map(|sep| split_once_word(&place, sep).map(|(_, tail)| tail))
            .unwrap_or(place);
        return (!project.is_empty()).then_some(TaskCommand::NewChat { project, instruction });
    }
    if let Some(verb) = OPEN_PROJECT_VERBS.iter().find(|v| lower.contains(**v)) {
        let rest = after(verb)?;
        // "<projeto>" or "<projeto> e <instrução>": open AND dispatch.
        let (query, instruction) = split_once_word(&rest, " e ")
            .or_else(|| split_once_word(&rest, " and "))
            .map(|(q, i)| (q, Some(i).filter(|s| !s.is_empty())))
            .unwrap_or((rest.trim().to_string(), None));
        return (!query.is_empty()).then_some(TaskCommand::OpenProject { query, instruction });
    }
    if let Some((_, tab)) = HQ_VERBS.iter().find(|(v, _)| lower.contains(*v)) {
        return Some(TaskCommand::OpenHq { tab: (*tab).to_string() });
    }
    // The settings SCREEN, never work: the noun is mandatory ("configura
    // o webhook" stays a dispatch).
    const SETTINGS_NOUNS: &[&str] = &[
        "configurações", "configuracoes", "as configuraç", "as configurac",
        "os ajustes", "as preferências", "as preferencias",
        "settings", "preferences",
    ];
    if ["abre", "abra", "abrir", "mostra", "open", "show"].iter().any(|v| lower.contains(v))
        && SETTINGS_NOUNS.iter().any(|n| lower.contains(n))
    {
        return Some(TaskCommand::OpenSettings);
    }
    if COMPACT_VERBS.iter().any(|v| lower.contains(v)) {
        return Some(TaskCommand::Compact);
    }
    if let Some(verb) = SET_MODE_VERBS.iter().find(|v| lower.contains(**v)) {
        let rest = after(verb)?.to_lowercase();
        return SET_MODE_TARGETS
            .iter()
            .find(|(needle, _)| rest.contains(needle))
            .map(|(_, mode)| TaskCommand::SetMode { mode: (*mode).to_string() });
    }
    if let Some(verb) = FIND_SESSION_VERBS.iter().find(|v| lower.contains(**v)) {
        let query = clean_topic(&after(verb)?);
        return (!query.is_empty()).then_some(TaskCommand::FindSession { query });
    }
    if let Some(verb) = SWITCH_VERBS.iter().find(|v| lower.contains(**v)) {
        let rest = after(verb)?;
        // "<query>" or "<query> e <instrução>"
        // "<query> e <instrução>" | "<query> and <instruction>": whichever
        // conjunction comes first ends the target and starts the work.
        let lower_rest = rest.to_lowercase();
        let cut = [" e ", " and "]
            .iter()
            .filter_map(|sep| lower_rest.find(sep).map(|i| (i, sep.len())))
            .min();
        let (query, instruction) = match cut {
            Some((i, len)) => (
                rest[..i].to_string(),
                Some(rest[i + len..].trim().to_string()).filter(|s| !s.is_empty()),
            ),
            None => (rest, None),
        };
        let query = clean_query(&query);
        return (!query.is_empty()).then_some(TaskCommand::Switch { query, instruction });
    }
    if let Some(verb) = RENAME_VERBS.iter().find(|v| lower.contains(**v)) {
        let rest = after(verb)?;
        let rest_lower = rest.to_lowercase();
        // "<query> para <novo>" — or just "para <novo>" / "esse chat para
        // <novo>": an empty query means "the focused one" (shell fills it).
        let (query, title) = if rest_lower.starts_with("para ") {
            (String::new(), rest["para ".len()..].trim().to_string())
        } else {
            let i = rest_lower.rfind(" para ")?;
            (rest[..i].to_string(), rest[i + " para ".len()..].trim().to_string())
        };
        let query = clean_rename_target(&query);
        return (!title.is_empty()).then_some(TaskCommand::Rename { query, title });
    }
    // Closing a task. Both halves are required: a verb AND the word
    // task/tarefa/chat as its object. The verb alone would swallow real
    // work — "fecha o PR", "conclui a migração" — and a board move that
    // eats a job is worse than no verb at all.
    if DONE_VERBS.iter().any(|v| {
        lower.split(|c: char| !c.is_alphanumeric()).any(|w| w == *v) || lower.contains(*v)
    }) {
        if let Some(object) = DONE_OBJECTS.iter().find(|o| {
            lower.split(|c: char| !c.is_alphanumeric()).any(|w| w == **o)
        }) {
            let rest = after(object).unwrap_or_default();
            // What follows the object is a NAME only if it is not just the
            // verb finishing the sentence ("mark this task done", "essa
            // task está concluída").
            let name = clean_query(&rest)
                .split_whitespace()
                .filter(|w| {
                    let w = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                    !w.is_empty()
                        && !DONE_VERBS.contains(&w.as_str())
                        && !FILLER.contains(&w.as_str())
                        && w != "está"
                        && w != "esta"
                })
                .collect::<Vec<_>>()
                .join(" ");
            return Some(TaskCommand::Done((!name.is_empty()).then_some(name)));
        }
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
    fn abre_o_chat_means_go_work_there() {
        // "abre o chat de X" is the user going BACK TO WORK on X — a
        // switch, not the read-only log viewer (19/08 incident phrasing).
        assert_eq!(
            parse("abre o chat de migração de assinaturas"),
            Some(TaskCommand::Switch {
                query: "migração de assinaturas".into(),
                instruction: None
            })
        );
        assert_eq!(
            parse("abre a thread dos alertas"),
            Some(TaskCommand::Switch { query: "alertas".into(), instruction: None })
        );
    }

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
    fn renames_the_focused_chat_without_naming_it() {
        // "this chat", "this task" or nothing at all: an empty query means
        // "whatever is focused right now" — the shell fills it in.
        assert_eq!(
            parse("renomeia esse chat para Migração dos alertas"),
            Some(TaskCommand::Rename {
                query: "".into(),
                title: "Migração dos alertas".into()
            })
        );
        assert_eq!(
            parse("renomeia essa task para Decom Carteira"),
            Some(TaskCommand::Rename {
                query: "".into(),
                title: "Decom Carteira".into()
            })
        );
        assert_eq!(
            parse("renomeia para Alertas do cluster"),
            Some(TaskCommand::Rename {
                query: "".into(),
                title: "Alertas do cluster".into()
            })
        );
        assert_eq!(
            parse("muda o título dessa sessão para Alertas do cluster"),
            Some(TaskCommand::Rename {
                query: "".into(),
                title: "Alertas do cluster".into()
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
    fn closing_the_task_you_are_in_is_a_command_not_work() {
        // "só queria fechar essa task" went to the agent instead, which
        // spent $7.68 probing a cluster that had already been torn down.
        for said in [
            "fechamos essa task?",
            "fecha essa task",
            "pode fechar essa task",
            "conclui essa task",
            "essa task está concluída",
            "terminamos essa tarefa",
            "close this task",
            "mark this task done",
        ] {
            assert_eq!(parse(said), Some(TaskCommand::Done(None)), "{said}");
        }
    }

    #[test]
    fn closing_a_task_by_name_names_it() {
        assert_eq!(
            parse("fecha a task dos alertas"),
            Some(TaskCommand::Done(Some("alertas".into())))
        );
    }

    #[test]
    fn closing_anything_that_is_not_a_task_is_real_work() {
        // The object matters: these are jobs for the agent, and hijacking
        // them into a board move would be worse than not having the verb.
        for said in [
            "fecha o PR",
            "fecha a conexão do banco",
            "conclui a migração de pagamentos",
            "fechar o modal quando clicar fora",
            "terminamos de escrever o teste",
        ] {
            assert_eq!(parse(said), None, "{said}");
        }
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
            parse("abre o arquivo readme do projeto hark"),
            Some(TaskCommand::OpenFile {
                query: "readme".into(),
                project: Some("hark".into())
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
            parse("abre o arquivo config.toml no projeto workspace-codigo"),
            Some(TaskCommand::OpenFile {
                query: "config.toml".into(),
                project: Some("workspace-codigo".into())
            })
        );
    }

    #[test]
    fn adds_projects_from_typed_or_spoken_paths() {
        assert_eq!(
            parse("adiciona o projeto ~/Projects/hark"),
            Some(TaskCommand::AddProject { path: "~/Projects/hark".into() })
        );
        assert_eq!(
            parse("adiciona o diretório home projects demo"),
            Some(TaskCommand::AddProject { path: "~/projects/demo".into() })
        );
    }

    #[test]
    fn starts_new_chats_inside_a_project() {
        assert_eq!(
            parse("novo chat no projeto hark"),
            Some(TaskCommand::NewChat { project: "hark".into(), instruction: None })
        );
        assert_eq!(
            parse("nova sessão no projeto workspace-codigo"),
            Some(TaskCommand::NewChat {
                project: "workspace-codigo".into(),
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
            parse("inicia um chat no workspace-codigo"),
            Some(TaskCommand::NewChat {
                project: "workspace-codigo".into(),
                instruction: None
            })
        );
        // "... para <instrução>" carries the first task of the chat.
        assert_eq!(
            parse("roda um chat no workspace código para reindexar as tasks perdidas"),
            Some(TaskCommand::NewChat {
                project: "workspace código".into(),
                instruction: Some("reindexar as tasks perdidas".into())
            })
        );
        assert_eq!(
            parse("novo chat no projeto hark para revisar o README"),
            Some(TaskCommand::NewChat {
                project: "hark".into(),
                instruction: Some("revisar o README".into())
            })
        );
    }

    #[test]
    fn opens_project_windows_with_optional_instruction() {
        assert_eq!(
            parse("abra o projeto hark"),
            Some(TaskCommand::OpenProject { query: "hark".into(), instruction: None })
        );
        // Works inside a longer sentence (STT never starts at the verb).
        assert_eq!(
            parse("eu quero que você abra o projeto workspace código"),
            Some(TaskCommand::OpenProject {
                query: "workspace código".into(),
                instruction: None
            })
        );
        // "... e <instrução>" opens AND dispatches inside it.
        assert_eq!(
            parse("abre o projeto workspace código e roda a reindexação das tasks"),
            Some(TaskCommand::OpenProject {
                query: "workspace código".into(),
                instruction: Some("roda a reindexação das tasks".into())
            })
        );
        // File commands keep priority even when they cite a project.
        assert_eq!(
            parse("abre o arquivo readme do projeto hark"),
            Some(TaskCommand::OpenFile {
                query: "readme".into(),
                project: Some("hark".into())
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

    #[test]
    fn a_new_project_by_voice_registers_the_spoken_path() {
        // The 19/08 miss: "abra um novo projeto em Projects barra
        // workspace" fell through to the active context and opened hark.
        assert_eq!(
            parse("abra um novo projeto em projects barra workspace"),
            Some(TaskCommand::AddProject { path: "~/projects/workspace".into() })
        );
        assert_eq!(
            parse("cria um projeto em home projects api"),
            Some(TaskCommand::AddProject { path: "~/projects/api".into() })
        );
        assert_eq!(
            parse("novo projeto no ~/Projects/tools"),
            Some(TaskCommand::AddProject { path: "~/Projects/tools".into() })
        );
        // "abre o projeto X" keeps meaning the REGISTERED project X.
        assert_eq!(
            parse("abre o projeto hark"),
            Some(TaskCommand::OpenProject { query: "hark".into(), instruction: None })
        );
    }

    #[test]
    fn opens_the_settings_by_voice() {
        assert_eq!(parse("abre as configurações"), Some(TaskCommand::OpenSettings));
        assert_eq!(parse("abra as configuracoes"), Some(TaskCommand::OpenSettings));
        assert_eq!(parse("abre os ajustes"), Some(TaskCommand::OpenSettings));
        assert_eq!(parse("mostra as configurações do hark"), Some(TaskCommand::OpenSettings));
        // "configura o webhook" is real work, not the settings screen.
        assert_eq!(parse("configura o webhook novo"), None);
    }

    #[test]
    fn compacts_the_context_of_the_focused_chat() {
        assert_eq!(parse("compacta o contexto"), Some(TaskCommand::Compact));
        assert_eq!(parse("por favor compacta a sessão"), Some(TaskCommand::Compact));
        assert_eq!(parse("compacta o chat"), Some(TaskCommand::Compact));
        assert_eq!(parse("faz a compactação do contexto"), Some(TaskCommand::Compact));
    }

    #[test]
    fn switches_the_permission_mode_by_voice() {
        let mode = |m: &str| Some(TaskCommand::SetMode { mode: m.into() });
        assert_eq!(parse("muda o modo pra automático"), mode("auto"));
        assert_eq!(parse("coloca no modo manual"), mode("manual"));
        assert_eq!(parse("troca o modo pra plano"), mode("plan"));
        assert_eq!(parse("muda o modo pra aceitar edições"), mode("acceptEdits"));
        assert_eq!(parse("muda o modo pra ignorar permissões"), mode("bypassPermissions"));
    }

    #[test]
    fn mode_and_compact_words_inside_real_work_fall_through() {
        // "modo automático" without a switching verb is a spawn directive,
        // not a command; "compactar arquivos" is real work.
        assert_eq!(parse("roda os testes em modo automático"), None);
        assert_eq!(parse("compactar os arquivos da pasta dist"), None);
        // A switching verb with no recognizable mode has nothing to do.
        assert_eq!(parse("muda o modo"), None);
    }
}

#[cfg(test)]
mod bilingual {
    use super::*;

    #[test]
    fn english_hq_tabs() {
        assert_eq!(parse("open the board"), Some(TaskCommand::OpenHq { tab: "board".into() }));
        assert_eq!(parse("show the costs"), Some(TaskCommand::OpenHq { tab: "custos".into() }));
    }

    #[test]
    fn english_project_windows_and_new_chats() {
        assert_eq!(
            parse("open the project webhook-api"),
            Some(TaskCommand::OpenProject { query: "webhook-api".into(), instruction: None })
        );
        assert_eq!(
            parse("new chat in webhook-api"),
            Some(TaskCommand::NewChat { project: "webhook-api".into(), instruction: None })
        );
    }

    #[test]
    fn english_switch_carries_the_instruction() {
        assert_eq!(
            parse("go to task billing endpoint and run the tests"),
            Some(TaskCommand::Switch {
                query: "billing endpoint".into(),
                instruction: Some("run the tests".into()),
            })
        );
    }

    #[test]
    fn english_log_viewer_stays_read_only() {
        assert_eq!(parse("show the log of billing"), Some(TaskCommand::Open("billing".into())));
    }

    #[test]
    fn english_settings_and_compaction() {
        assert_eq!(parse("open the settings"), Some(TaskCommand::OpenSettings));
        assert_eq!(parse("compact the context"), Some(TaskCommand::Compact));
    }

    #[test]
    fn english_session_recovery_and_files() {
        assert!(matches!(
            parse("find the session dns cleanup"),
            Some(TaskCommand::FindSession { .. })
        ));
        assert!(matches!(
            parse("open the file src/main.rs"),
            Some(TaskCommand::OpenFile { .. })
        ));
    }
}

#[cfg(test)]
mod incident_2408 {
    use super::*;

    #[test]
    fn buscar_finds_a_session_the_way_procurar_does() {
        // "busca" was simply absent from the list while "procura", "acha"
        // and "encontra" were there. Same verb, same intent, no command.
        for verb in ["busca", "procura", "acha", "encontra"] {
            let said = format!("{verb} no histórico o chat pin vigia chart");
            assert!(
                matches!(parse(&said), Some(TaskCommand::FindSession { .. })),
                "{said:?} should look up a session"
            );
        }
    }

    #[test]
    fn a_trailing_open_clause_is_not_part_of_the_topic() {
        // Spoken: "…chamado pin vigia chart last version E ABRA ELE".
        // The tail is punctuation, not the session's name.
        match parse("busca o chat pin vigia chart last version e abra ele") {
            Some(TaskCommand::FindSession { query }) => {
                assert!(!query.contains("abra"), "tail leaked into the query: {query:?}");
                assert!(query.contains("vigia"), "topic survived: {query:?}");
            }
            other => panic!("expected a session lookup, got {other:?}"),
        }
    }
}
