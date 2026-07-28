use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct GitOutput {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

pub fn run_git(
    args: &[&str],
    cwd: Option<&Path>,
    input: Option<&[u8]>,
    check: bool,
) -> Result<GitOutput, String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn git: {e}"))?;

    if let Some(data) = input {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(data)
                .map_err(|e| format!("failed to write git stdin: {e}"))?;
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for git: {e}"))?;

    let status = output.status.code().unwrap_or(1);
    let stdout = output.stdout;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if check && status != 0 {
        let stdout_s = String::from_utf8_lossy(&stdout);
        return Err(format!(
            "git {} failed ({status}): {}",
            args.join(" "),
            if !stderr.is_empty() {
                stderr.trim()
            } else {
                stdout_s.trim()
            }
        ));
    }

    Ok(GitOutput {
        status,
        stdout,
        stderr,
    })
}

pub fn git_rev_parse_top_level(cwd: &Path) -> Result<PathBuf, String> {
    let out = run_git(&["rev-parse", "--show-toplevel"], Some(cwd), None, true)?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        return Err(format!("empty toplevel for {}", cwd.display()));
    }
    Ok(PathBuf::from(s))
}

/// First worktree path from `git worktree list --porcelain` (main checkout).
pub fn git_main_worktree_root(cwd: &Path) -> Option<PathBuf> {
    let out = run_git(
        &["worktree", "list", "--porcelain"],
        Some(cwd),
        None,
        false,
    )
    .ok()?;
    if out.status != 0 {
        return None;
    }
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            return Some(PathBuf::from(path));
        }
    }
    None
}

/// All linked worktree absolute paths for a repository.
pub fn git_worktree_paths(cwd: &Path) -> Vec<PathBuf> {
    let Ok(out) = run_git(
        &["worktree", "list", "--porcelain"],
        Some(cwd),
        None,
        false,
    ) else {
        return Vec::new();
    };
    if out.status != 0 {
        return Vec::new();
    }
    let mut paths = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            paths.push(PathBuf::from(path));
        }
    }
    paths
}

/// Null-delimited paths from stdout.
pub fn split_null(stdout: &[u8]) -> Vec<String> {
    if stdout.is_empty() {
        return Vec::new();
    }
    stdout
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}
