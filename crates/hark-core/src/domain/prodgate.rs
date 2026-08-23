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
/// auto-approval may answer it. Bash only — that is where infra happens.
pub fn check(tool_name: &str, input_json: &str) -> Option<String> {
    if tool_name != "Bash" {
        return None;
    }
    let command: String = serde_json::from_str::<serde_json::Value>(input_json)
        .ok()
        .and_then(|v| v.get("command").and_then(|c| c.as_str()).map(String::from))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn bash(command: &str) -> String {
        serde_json::json!({ "command": command }).to_string()
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
