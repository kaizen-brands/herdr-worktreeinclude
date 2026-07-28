use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::git::{
    git_main_worktree_root, git_rev_parse_top_level, git_worktree_paths, run_git, split_null,
};

const INCLUDE_NAME: &str = ".worktreeinclude";

#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    /// When true, plan only — no filesystem writes.
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ApplyResult {
    pub include_file: Option<PathBuf>,
    pub matched: usize,
    pub copied: Vec<String>,
    pub skipped_existing: Vec<String>,
    pub skipped_tracked: Vec<String>,
    pub skipped_missing: Vec<String>,
    pub skipped_other_worktree: Vec<String>,
    pub skipped_unsafe: Vec<String>,
}

/// Copy files that match `.worktreeinclude` AND are git-ignored from source
/// into target. Tracked files are never copied. Existing destinations are skipped.
pub fn apply_worktree_include(options: &ApplyOptions) -> Result<ApplyResult, String> {
    let source_root = git_rev_parse_top_level(&options.source_root)?;
    let target_root = git_rev_parse_top_level(&options.target_root)?;
    let include_file = source_root.join(INCLUDE_NAME);

    let empty = ApplyResult::default();

    let include_contents = match fs::read_to_string(&include_file) {
        Ok(s) => s,
        Err(_) => return Ok(empty),
    };

    // Evaluate patterns in an empty repo so source .gitignore cannot override
    // .worktreeinclude negations.
    let pattern_dir = tempfile::Builder::new()
        .prefix("herdr-wti-patterns-")
        .tempdir()
        .map_err(|e| format!("tempdir: {e}"))?;
    let pattern_root = pattern_dir.path();

    run_git(&["init", "-q"], Some(pattern_root), None, true)?;
    fs::write(pattern_root.join(".gitignore"), &include_contents)
        .map_err(|e| format!("write pattern .gitignore: {e}"))?;

    let ignored = run_git(
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
        ],
        Some(&source_root),
        None,
        false,
    )?;
    if ignored.status != 0 {
        let detail = if !ignored.stderr.is_empty() {
            ignored.stderr
        } else {
            String::from_utf8_lossy(&ignored.stdout).into_owned()
        };
        return Err(format!(
            "git ls-files failed in {}: {}",
            source_root.display(),
            detail.trim()
        ));
    }

    let matched = run_git(
        &["check-ignore", "--no-index", "--stdin", "-z"],
        Some(pattern_root),
        Some(&ignored.stdout),
        false,
    )?;
    // check-ignore exits 1 when nothing matches; still fine.
    if matched.status != 0 && matched.status != 1 {
        let detail = if !matched.stderr.is_empty() {
            matched.stderr
        } else {
            String::from_utf8_lossy(&matched.stdout).into_owned()
        };
        return Err(format!("git check-ignore failed: {}", detail.trim()));
    }

    let candidates = split_null(&matched.stdout);
    let other_worktrees: Vec<PathBuf> = git_worktree_paths(&source_root)
        .into_iter()
        .filter(|p| canonicalize_loose(p) != canonicalize_loose(&source_root))
        .collect();

    let tracked_out = run_git(&["ls-files", "-z"], Some(&target_root), None, false)?;
    let tracked: HashSet<String> = split_null(&tracked_out.stdout).into_iter().collect();

    let mut result = ApplyResult {
        include_file: Some(include_file),
        matched: candidates.len(),
        ..Default::default()
    };

    for relative_path in candidates {
        if !is_safe_relative_path(&relative_path) {
            result.skipped_unsafe.push(relative_path);
            continue;
        }

        let source_path = source_root.join(&relative_path);
        let destination_path = target_root.join(&relative_path);

        if is_under_other_worktree(&source_path, &other_worktrees) {
            result.skipped_other_worktree.push(relative_path);
            continue;
        }

        let source_meta = match fs::symlink_metadata(&source_path) {
            Ok(m) => m,
            Err(_) => {
                result.skipped_missing.push(relative_path);
                continue;
            }
        };

        if tracked.contains(&relative_path) {
            result.skipped_tracked.push(relative_path);
            continue;
        }

        if fs::symlink_metadata(&destination_path).is_ok() {
            result.skipped_existing.push(relative_path);
            continue;
        }

        if options.dry_run {
            result.copied.push(relative_path);
            continue;
        }

        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }

        copy_path(&source_path, &destination_path, &source_meta)
            .map_err(|e| format!("copy {}: {e}", relative_path))?;
        result.copied.push(relative_path);
    }

    Ok(result)
}

/// Resolve source for a new worktree: prefer explicit source, else main worktree.
pub fn resolve_source_root(
    target_root: &Path,
    preferred_source: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(src) = preferred_source {
        if !src.is_empty() {
            if let Ok(root) = git_rev_parse_top_level(Path::new(src)) {
                return Ok(root);
            }
        }
    }
    git_main_worktree_root(target_root)
        .ok_or_else(|| format!("Could not resolve main worktree for {}", target_root.display()))
}

fn is_safe_relative_path(relative_path: &str) -> bool {
    if relative_path.is_empty()
        || relative_path.starts_with('/')
        || relative_path.contains('\0')
    {
        return false;
    }
    let normalized = relative_path.replace('\\', "/");
    if normalized == ".." || normalized.starts_with("../") || normalized.contains("/../") {
        return false;
    }
    // Reject absolute-like Windows paths if they ever appear
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    true
}

fn is_under_other_worktree(source_path: &Path, other_worktrees: &[PathBuf]) -> bool {
    let abs = canonicalize_loose(source_path);
    for wt in other_worktrees {
        let root = canonicalize_loose(wt);
        // Path::starts_with is component-wise on Unix (not raw string prefix).
        if abs == root || abs.starts_with(&root) {
            return true;
        }
    }
    false
}

fn canonicalize_loose(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn copy_path(source: &Path, dest: &Path, meta: &fs::Metadata) -> io::Result<()> {
    let ft = meta.file_type();
    if ft.is_symlink() {
        let target = fs::read_link(source)?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, dest)?;
        }
        #[cfg(not(unix))]
        {
            // Fallback: copy contents
            if meta.is_dir() {
                copy_dir_recursive(source, dest)?;
            } else {
                fs::copy(source, dest)?;
            }
        }
        return Ok(());
    }
    if ft.is_dir() {
        return copy_dir_recursive(source, dest);
    }
    fs::copy(source, dest)?;
    Ok(())
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if ty.is_dir() && !ty.is_symlink() {
            copy_dir_recursive(&from, &to)?;
        } else if ty.is_symlink() {
            let target = fs::read_link(&from)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, &to)?;
            #[cfg(not(unix))]
            {
                if from.is_dir() {
                    copy_dir_recursive(&from, &to)?;
                } else {
                    fs::copy(&from, &to)?;
                }
            }
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
