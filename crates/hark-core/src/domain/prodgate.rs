//! The production gate: commands that touch REAL infrastructure never get
//! auto-approved, whatever the permission mode. Standing "always allow"
//! rules and permissive modes cover the boring 95%; this is the hard floor
//! under the dangerous 5% — a human answers, every time.

/// Tool rules injected as `--disallowedTools` when a worker spawns in
/// bypassPermissions: that mode never asks, so the infra CLIs are blocked
/// outright — the model is told why and relays it, instead of silently
/// touching production.
pub const BYPASS_DENY_RULES: &[&str] = &[
    "Bash(kubectl:*)",
    "Bash(terraform:*)",
    "Bash(helm:*)",
    "Bash(flyctl:*)",
];

/// Does this tool call touch production-grade state? `Some(reason)` means
/// the ask must reach a human: no standing rule, no permissive mode, no
/// auto-approval may answer it. Shell commands only — that is where infra
/// happens. Which tool is the shell is the plugin's sheet (`shell_tools`:
/// claude's `Bash`, an ACP agent's `execute` kind); under the sheet, any
/// input carrying a `command` is read as one too — a false positive is
/// one more human click, a false negative is terraform apply in prod.
pub fn check(tool_name: &str, input_json: &str, shell_tools: &[String]) -> Option<String> {
    let input: serde_json::Value = serde_json::from_str(input_json).ok()?;
    let command = command_of(&input)?;
    if !shell_tools.iter().any(|t| t == tool_name) && input.get("command").is_none() {
        return None;
    }
    let lower = command.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let has = |w: &str| words.contains(&w);
    let after = |tool: &str, verbs: &[&str]| {
        has(tool) && verbs.iter().any(|v| has(v))
    };

    if after("kubectl", &["apply", "delete", "scale", "rollout", "drain", "cordon", "patch", "replace", "edit"]) {
        return Some("kubectl que altera o cluster".into());
    }
    if after("terraform", &["apply", "destroy"]) || after("tofu", &["apply", "destroy"]) {
        return Some("terraform no ambiente real".into());
    }
    if after("helm", &["install", "upgrade", "uninstall", "rollback", "delete"]) {
        return Some("helm mexendo em release".into());
    }
    if has("git") && has("push") && (has("--force") || has("-f") || has("--force-with-lease")) {
        return Some("force push reescreve histórico remoto".into());
    }
    if lower.contains("drop table") || lower.contains("drop database") || lower.contains("truncate") {
        return Some("SQL destrutivo".into());
    }
    if lower.contains("delete from") && !lower.contains(" where ") {
        return Some("DELETE sem WHERE".into());
    }
    if after("aws", &["terminate-instances", "delete", "rm", "deregister"])
        || after("gcloud", &["delete"])
        || after("az", &["delete"])
    {
        return Some("remoção de recurso na nuvem".into());
    }
    if (has("npm") || has("cargo") || has("gem")) && has("publish") {
        return Some("publica pacote público".into());
    }
    if has("docker") && has("push") {
        return Some("publica imagem no registry".into());
    }
    if after("flyctl", &["deploy"]) || after("fly", &["deploy"]) || (has("vercel") && has("--prod")) {
        return Some("deploy em produção".into());
    }
    if has("rm") && (has("-rf") || has("-fr")) && words.iter().any(|w| *w == "/" || *w == "~") {
        return Some("apaga a raiz do disco".into());
    }
    None
}

/// The command line of a tool input: a string, or an argv array joined
/// (codex-acp hands `["bash", "-lc", "…"]`).
fn command_of(input: &serde_json::Value) -> Option<String> {
    match input.get("command")? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bash(command: &str) -> String {
        serde_json::json!({ "command": command }).to_string()
    }

    /// claude's sheet: the shell tool is `Bash`.
    const CLAUDE: &[&str] = &["Bash"];

    fn check(tool: &str, input: &str) -> Option<String> {
        let sheet: Vec<String> = CLAUDE.iter().map(|s| s.to_string()).collect();
        super::check(tool, input, &sheet)
    }

    #[test]
    fn an_acp_execute_tool_is_a_shell_tool_when_the_sheet_says_so() {
        // Over ACP the agent's shell tool is whatever the agent calls it;
        // the plugin reports the ACP `kind` ("execute") and the sheet
        // names it — the gate must fire there exactly as on Bash.
        let sheet = vec!["execute".to_string()];
        assert!(super::check("execute", &bash("kubectl apply -f deploy.yaml"), &sheet).is_some());
        assert_eq!(super::check("execute", &bash("kubectl get pods"), &sheet), None);
        // The sheet decides the name: with claude's sheet, "execute" is not a shell tool by name…
        // …but a shell-looking input still is (below).
    }

    #[test]
    fn a_command_given_as_an_argv_array_is_read_as_one_line() {
        // codex-acp's shell tool hands the command as argv.
        let input = serde_json::json!({ "command": ["bash", "-lc", "terraform apply -auto-approve"], "workdir": "/p" }).to_string();
        assert!(super::check("execute", &input, &["execute".to_string()]).is_some());
    }

    #[test]
    fn a_shell_looking_input_is_gated_whatever_the_tool_is_called() {
        // The belt under the sheet: an input with a `command` is a shell
        // command whoever runs it. A false positive is one extra human
        // click; a false negative is terraform apply in production.
        assert!(super::check("run_shell_command", &bash("helm uninstall api"), &[]).is_some());
        assert_eq!(super::check("run_shell_command", &bash("ls -la"), &[]), None);
    }

    #[test]
    fn a_tool_with_no_command_in_it_is_not_a_shell_tool() {
        let read = serde_json::json!({ "path": "/etc/hosts" }).to_string();
        assert_eq!(super::check("read_file", &read, &["execute".to_string()]), None);
        assert_eq!(super::check("Read", &read, &["Bash".to_string()]), None);
    }

    #[test]
    fn cluster_mutations_are_flagged() {
        for cmd in [
            "kubectl apply -f deploy.yaml",
            "kubectl delete pod api-7f9",
            "kubectl rollout restart deployment/api",
            "kubectl scale deployment api --replicas=0",
            "kubectl drain node-3",
            "helm upgrade api ./chart",
            "helm uninstall api",
            "terraform apply",
            "terraform destroy -auto-approve",
        ] {
            assert!(check("Bash", &bash(cmd)).is_some(), "{cmd} must be gated");
        }
    }

    #[test]
    fn reads_and_plans_pass_free() {
        for cmd in [
            "kubectl get pods -A",
            "kubectl describe deployment api",
            "kubectl logs api-7f9 -f",
            "terraform plan",
            "helm list",
            "git status",
            "aws s3 ls",
        ] {
            assert_eq!(check("Bash", &bash(cmd)), None, "{cmd} must pass");
        }
    }

    #[test]
    fn force_push_is_flagged_normal_push_is_not() {
        assert!(check("Bash", &bash("git push --force origin main")).is_some());
        assert!(check("Bash", &bash("git push -f")).is_some());
        assert_eq!(check("Bash", &bash("git push origin feature/x")), None);
    }

    #[test]
    fn destructive_sql_is_flagged() {
        for cmd in [
            "psql -c 'DROP TABLE users'",
            "mysql -e \"TRUNCATE orders\"",
            "psql -c 'delete from sessions'",
        ] {
            assert!(check("Bash", &bash(cmd)).is_some(), "{cmd} must be gated");
        }
        // A scoped delete is routine work.
        assert_eq!(
            check("Bash", &bash("psql -c 'DELETE FROM sessions WHERE id = 3'")),
            None
        );
    }

    #[test]
    fn publishes_and_cloud_deletes_are_flagged() {
        for cmd in [
            "npm publish",
            "cargo publish",
            "docker push registry.io/api:latest",
            "aws ec2 terminate-instances --instance-ids i-1",
            "aws s3 rm s3://bucket --recursive",
            "gcloud compute instances delete vm-1",
            "flyctl deploy",
            "vercel --prod",
        ] {
            assert!(check("Bash", &bash(cmd)).is_some(), "{cmd} must be gated");
        }
    }

    #[test]
    fn wiping_the_disk_is_flagged() {
        assert!(check("Bash", &bash("rm -rf /")).is_some());
        assert!(check("Bash", &bash("rm -rf ~")).is_some());
        assert_eq!(check("Bash", &bash("rm -rf target/")), None);
    }

    #[test]
    fn other_tools_are_not_the_gates_business() {
        assert_eq!(check("Edit", r#"{"file_path":"/tmp/x"}"#), None);
        assert_eq!(check("Write", r#"{"content":"kubectl apply"}"#), None);
    }

    #[test]
    fn reasons_are_short_pt_br() {
        let reason = check("Bash", &bash("terraform apply")).unwrap();
        assert!(reason.chars().count() < 60);
        assert!(!reason.is_empty());
    }

    #[test]
    fn bypass_deny_rules_cover_the_infra_tools() {
        for rule in ["Bash(kubectl:*)", "Bash(terraform:*)", "Bash(helm:*)"] {
            assert!(BYPASS_DENY_RULES.contains(&rule), "missing {rule}");
        }
    }
}
