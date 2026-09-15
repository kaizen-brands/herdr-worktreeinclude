use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use herdr_worktreeinclude::git::git_rev_parse_top_level;
use serde_json::Value;

const HARNESSES: &[&str] = &[
    "claude",
    "codex",
    "opencode",
    "pi",
    "codex-opencode",
    "codex-openrouter",
    "grok",
    "automation",
];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CreateOptions {
    pub harness: Option<String>,
    pub slug: Option<String>,
}

/// Create a canonical Kaizen worktree, then register and open it in Herdr.
pub fn create_and_open(options: &CreateOptions) -> Result<PathBuf, String> {
    let context = context_json();
    let context_path = context
        .as_ref()
        .and_then(resolve_context_path)
        .or_else(|| env::var("HERDR_WORKSPACE_CWD").ok())
        .or_else(|| env::var("PWD").ok())
        .ok_or_else(|| "could not resolve the current Herdr workspace path".to_string())?;
    let repo_root = git_rev_parse_top_level(Path::new(&context_path))?;

    let slug_raw = options
        .slug
        .clone()
        .or_else(|| env::var("KAIZEN_HERDR_WORKTREE_SLUG").ok())
        .or_else(|| context.as_ref().and_then(resolve_context_label))
        .or_else(|| {
            repo_root
                .file_name()
                .and_then(|name| name.to_str())
                .map(String::from)
        });
    let slug = sanitize_component(slug_raw.as_deref().unwrap_or("worktree"));

    let harness = options
        .harness
        .clone()
        .or_else(|| env::var("KAIZEN_HERDR_HARNESS").ok())
        .or_else(|| env::var("KAIZEN_WORKTREE_HARNESS").ok())
        .unwrap_or_else(detect_harness);
    validate_harness(&harness)?;

    let helper = resolve_helper()?;
    let helper_args = vec![
        "create".to_string(),
        "--repo".to_string(),
        repo_root.to_string_lossy().into_owned(),
        "--harness".to_string(),
        harness.clone(),
        "--slug".to_string(),
        slug.clone(),
    ];
    let helper_output = Command::new(&helper)
        .args(&helper_args)
        .current_dir(&repo_root)
        .output()
        .map_err(|err| format!("could not run {}: {err}", helper.display()))?;
    if !helper_output.status.success() {
        return Err(format_command_failure(
            "canonical worktree creation",
            &helper_output,
        ));
    }

    let destination = last_nonempty_line(&helper_output.stdout)
        .ok_or_else(|| "canonical worktree creator returned no destination path".to_string())?;
    let destination = PathBuf::from(destination);

    let herdr = env::var_os("HERDR_BIN_PATH").unwrap_or_else(|| "herdr".into());
    let open_args = vec![
        "worktree".to_string(),
        "open".to_string(),
        "--cwd".to_string(),
        repo_root.to_string_lossy().into_owned(),
        "--path".to_string(),
        destination.to_string_lossy().into_owned(),
        "--label".to_string(),
        slug,
        "--no-focus".to_string(),
    ];
    let open_output = Command::new(&herdr)
        .args(&open_args)
        .current_dir(&repo_root)
        .output()
        .map_err(|err| format!("could not run Herdr: {err}"))?;
    if !open_output.status.success() {
        return Err(format_command_failure(
            "opening the Kaizen worktree in Herdr",
            &open_output,
        ));
    }

    println!("kaizen worktree: {} ({harness})", destination.display());
    Ok(destination)
}

pub fn parse_create_options(args: &[String]) -> Result<CreateOptions, String> {
    let mut options = CreateOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--harness" => {
                index += 1;
                options.harness = Some(
                    args.get(index)
                        .cloned()
                        .ok_or_else(|| "--harness requires a value".to_string())?,
                );
            }
            "--slug" => {
                index += 1;
                options.slug = Some(
                    args.get(index)
                        .cloned()
                        .ok_or_else(|| "--slug requires a value".to_string())?,
                );
            }
            "--help" | "-h" => {
                return Err(
                    "usage: herdr-worktreeinclude create-kaizen-worktree [--harness NAME] [--slug SLUG]"
                        .to_string(),
                );
            }
            other => return Err(format!("unknown create-kaizen-worktree option: {other}")),
        }
        index += 1;
    }
    Ok(options)
}

fn context_json() -> Option<Value> {
    env::var("HERDR_PLUGIN_CONTEXT_JSON")
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

fn resolve_context_path(context: &Value) -> Option<String> {
    [
        &["workspace_cwd"][..],
        &["focused_pane_cwd"][..],
        &["cwd"][..],
        &["workspace", "cwd"][..],
        &["workspace", "path"][..],
        &["worktree", "checkout_path"][..],
        &["worktree", "path"][..],
    ]
    .iter()
    .find_map(|path| dig_str(context, path))
}

fn resolve_context_label(context: &Value) -> Option<String> {
    [
        &["workspace", "label"][..],
        &["workspace", "name"][..],
        &["label"][..],
        &["name"][..],
    ]
    .iter()
    .find_map(|path| dig_str(context, path))
}

fn dig_str(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .filter(|value| !value.is_empty())
        .map(String::from)
}

fn detect_harness() -> String {
    if env::var("PI_CODING_AGENT")
        .map(|value| value == "true" || value == "1")
        .unwrap_or(false)
    {
        return "pi".to_string();
    }
    if env::var("CLAUDECODE").ok().as_deref() == Some("1") {
        return "claude".to_string();
    }
    match env::var("CODEX_HOME").ok().as_deref() {
        Some(path) if path.ends_with("/.codex-opencode") => "codex-opencode".to_string(),
        Some(path) if path.ends_with("/.codex-openrouter") => "codex-openrouter".to_string(),
        _ => "automation".to_string(),
    }
}

fn validate_harness(harness: &str) -> Result<(), String> {
    if HARNESSES.contains(&harness) {
        Ok(())
    } else {
        Err(format!(
            "unsupported harness {harness:?}; expected one of {}",
            HARNESSES.join(", ")
        ))
    }
}

fn resolve_helper() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("KAIZEN_WORKTREE_HELPER") {
        candidates.push(PathBuf::from(path));
    }
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join(".local/lib/kaizen-worktrees/kaizen-worktree.sh"));
        candidates.push(home.join("Kaizen/scripts/worktrees/kaizen-worktree.sh"));
    }
    candidates.into_iter().find(|path| path.is_file()).ok_or_else(|| {
        "could not find Kaizen's canonical worktree helper; set KAIZEN_WORKTREE_HELPER or install the Kaizen worktree runtime".to_string()
    })
}

fn sanitize_component(value: &str) -> String {
    let mut result = String::new();
    let mut pending_separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            if pending_separator && !result.is_empty() {
                result.push('-');
            }
            pending_separator = false;
            result.push(character);
        } else {
            pending_separator = true;
        }
    }
    let result = result.trim_matches('-').to_string();
    if result.is_empty() {
        "worktree".to_string()
    } else {
        result
    }
}

fn last_nonempty_line(output: &[u8]) -> Option<String> {
    String::from_utf8_lossy(output)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(String::from)
}

fn format_command_failure(label: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if stderr.is_empty() { stdout } else { stderr };
    if detail.is_empty() {
        format!("{label} failed with status {}", output.status)
    } else {
        format!("{label} failed: {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn context_prefers_workspace_cwd_and_label() {
        let context = json!({
            "workspace_cwd": "/repo",
            "workspace": {"cwd": "/wrong", "label": "Checkout Fix"}
        });
        assert_eq!(resolve_context_path(&context).as_deref(), Some("/repo"));
        assert_eq!(
            resolve_context_label(&context).as_deref(),
            Some("Checkout Fix")
        );
    }

    #[test]
    fn sanitizes_labels_into_helper_components() {
        assert_eq!(sanitize_component("Checkout Fix / API"), "Checkout-Fix-API");
        assert_eq!(sanitize_component("---"), "worktree");
    }

    #[test]
    fn parses_create_options() {
        let args = vec![
            "--harness".to_string(),
            "codex".to_string(),
            "--slug".to_string(),
            "preview".to_string(),
        ];
        assert_eq!(
            parse_create_options(&args).unwrap(),
            CreateOptions {
                harness: Some("codex".to_string()),
                slug: Some("preview".to_string()),
            }
        );
    }
}
