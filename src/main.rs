//! herdr-worktreeinclude: restore `.worktreeinclude`-selected gitignored files
//! into new Git worktrees.
//!
//! One binary, dispatched by subcommand (set in herdr-plugin.toml):
//!   on-worktree-created   event hook for worktree.created
//!   apply                 manual action for the focused worktree
//!   create-kaizen-worktree create and open a canonical Kaizen worktree

use std::env;
use std::path::{Path, PathBuf};
use std::process;

use herdr_worktreeinclude::apply::{apply_worktree_include, resolve_source_root, ApplyOptions};
use herdr_worktreeinclude::git::git_rev_parse_top_level;
use herdr_worktreeinclude::parse_event::parse_worktree_created_paths;
use serde_json::Value;

mod kaizen;

fn main() {
    let args: Vec<String> = env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("on-worktree-created") => cmd_on_worktree_created(),
        Some("apply") => cmd_apply(&args[2..]),
        Some("create-kaizen-worktree") => cmd_create_kaizen_worktree(&args[2..]),
        other => {
            eprintln!(
                "usage: herdr-worktreeinclude <on-worktree-created | apply [--dry-run] | create-kaizen-worktree [--harness NAME] [--slug SLUG]>"
            );
            eprintln!("got: {other:?}");
            2
        }
    };
    process::exit(code);
}

fn cmd_create_kaizen_worktree(args: &[String]) -> i32 {
    let options = match kaizen::parse_create_options(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("worktreeinclude: {error}");
            return 2;
        }
    };

    match kaizen::create_and_open(&options) {
        Ok(_) => 0,
        Err(error) => {
            eprintln!("worktreeinclude: {error}");
            1
        }
    }
}

fn log(message: &str) {
    eprintln!("worktreeinclude: {message}");
}

/// Event hook: after worktree.created, copy matching files. Prefer exit 0 on
/// soft failures so worktree creation is not blocked.
fn cmd_on_worktree_created() -> i32 {
    let raw = env::var("HERDR_PLUGIN_EVENT_JSON").ok();
    let paths = parse_worktree_created_paths(raw.as_deref());
    let Some(paths) = paths else {
        log("no worktree path in HERDR_PLUGIN_EVENT_JSON; skip");
        return 0;
    };
    if paths.target_root.is_empty() {
        log("no worktree path in HERDR_PLUGIN_EVENT_JSON; skip");
        return 0;
    }

    let target = PathBuf::from(&paths.target_root);
    let source_root = match resolve_source_root(
        &target,
        if paths.source_root.is_empty() {
            None
        } else {
            Some(paths.source_root.as_str())
        },
    ) {
        Ok(p) => p,
        Err(err) => {
            log(&format!("could not resolve source worktree: {err}"));
            return 0;
        }
    };

    match apply_worktree_include(&ApplyOptions {
        source_root: source_root.clone(),
        target_root: target.clone(),
        dry_run: false,
    }) {
        Ok(result) => {
            if result.include_file.is_none() {
                return 0;
            }
            log(&format!(
                "{} → {}: copied {}, skipped existing {}, tracked {}, missing {}",
                source_root.display(),
                target.display(),
                result.copied.len(),
                result.skipped_existing.len(),
                result.skipped_tracked.len(),
                result.skipped_missing.len(),
            ));
            for p in &result.copied {
                log(&format!("copied: {p}"));
            }
            0
        }
        Err(err) => {
            log(&format!("apply failed: {err}"));
            // Do not fail worktree creation hard for unexpected errors in v0.1
            0
        }
    }
}

/// Manual action: apply .worktreeinclude into the focused worktree.
fn cmd_apply(args: &[String]) -> i32 {
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let target_root = resolve_target_from_env()
        .or_else(|| env::current_dir().ok().and_then(|d| git_rev_parse_top_level(&d).ok()));

    let Some(target_root) = target_root else {
        eprintln!("worktreeinclude: could not resolve target worktree from context or cwd");
        return 1;
    };

    let source_root = {
        let event = parse_worktree_created_paths(env::var("HERDR_PLUGIN_EVENT_JSON").ok().as_deref());
        let preferred = event
            .as_ref()
            .map(|e| e.source_root.as_str())
            .filter(|s| !s.is_empty());
        match resolve_source_root(&target_root, preferred) {
            Ok(p) => p,
            Err(err) => {
                eprintln!("worktreeinclude: {err}");
                return 1;
            }
        }
    };

    let result = match apply_worktree_include(&ApplyOptions {
        source_root: source_root.clone(),
        target_root: target_root.clone(),
        dry_run,
    }) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("worktreeinclude: {err}");
            return 1;
        }
    };

    if result.include_file.is_none() {
        println!(
            "No .worktreeinclude in {}; nothing to do.",
            source_root.display()
        );
        return 0;
    }

    let verb = if dry_run { "would copy" } else { "copied" };
    println!(
        "{verb} {} path(s); skipped existing={} tracked={} missing={}",
        result.copied.len(),
        result.skipped_existing.len(),
        result.skipped_tracked.len(),
        result.skipped_missing.len(),
    );
    for p in &result.copied {
        println!("  {verb}: {p}");
    }
    0
}

fn resolve_target_from_env() -> Option<PathBuf> {
    if let Ok(raw) = env::var("HERDR_PLUGIN_CONTEXT_JSON") {
        if let Ok(ctx) = serde_json::from_str::<Value>(&raw) {
            let candidates = [
                dig_str(&ctx, &["worktree", "checkout_path"]),
                dig_str(&ctx, &["worktree", "path"]),
                dig_str(&ctx, &["workspace_cwd"]),
                dig_str(&ctx, &["focused_pane_cwd"]),
            ];
            for c in candidates.into_iter().flatten() {
                if let Ok(root) = git_rev_parse_top_level(Path::new(&c)) {
                    return Some(root);
                }
            }
        }
    }

    for key in ["HERDR_WORKSPACE_CWD", "PWD"] {
        if let Ok(v) = env::var(key) {
            if let Ok(root) = git_rev_parse_top_level(Path::new(&v)) {
                return Some(root);
            }
        }
    }
    None
}

fn dig_str(obj: &Value, path: &[&str]) -> Option<String> {
    let mut cur = obj;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}
