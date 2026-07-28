use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use herdr_worktreeinclude::apply::{apply_worktree_include, ApplyOptions};

fn run_git(args: &[&str], cwd: &Path) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {args:?} failed");
}

fn run_git_ok(args: &[&str], cwd: &Path) {
    let _ = Command::new("git").args(args).current_dir(cwd).status();
}

fn init_repo(dir: &Path) {
    run_git(&["init", "-q"], dir);
    run_git(&["config", "user.email", "test@example.com"], dir);
    run_git(&["config", "user.name", "test"], dir);
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

struct Fixture {
    _tmp: tempfile::TempDir,
    main: PathBuf,
    worktree: PathBuf,
}

fn setup() -> Fixture {
    let tmp = tempfile::Builder::new()
        .prefix("herdr-wti-test-")
        .tempdir()
        .unwrap();
    let main = tmp.path().join("main");
    let worktree = tmp.path().join("feature");
    fs::create_dir(&main).unwrap();

    init_repo(&main);
    write(&main.join("README.md"), "hello\n");
    write(
        &main.join(".gitignore"),
        ".env\n.env.local\nconfig/secrets.json\n",
    );
    write(&main.join(".env"), "SECRET=1\n");
    write(&main.join(".env.local"), "LOCAL=1\n");
    write(&main.join(".env.example"), "EXAMPLE=1\n");
    write(&main.join("config/secrets.json"), "{\"k\":1}\n");
    write(&main.join("config/app.json"), "{\"app\":true}\n");
    write(
        &main.join(".worktreeinclude"),
        ".env\n.env.local\nconfig/secrets.json\n!.env.example\n",
    );

    run_git(
        &[
            "add",
            "README.md",
            ".gitignore",
            ".env.example",
            "config/app.json",
            ".worktreeinclude",
        ],
        &main,
    );
    run_git_ok(&["add", "-f", ".env.example"], &main);
    run_git(&["commit", "-q", "-m", "init"], &main);
    run_git(&["branch", "feature"], &main);
    run_git(
        &[
            "worktree",
            "add",
            "-q",
            worktree.to_str().unwrap(),
            "feature",
        ],
        &main,
    );

    Fixture {
        _tmp: tmp,
        main,
        worktree,
    }
}

#[test]
fn copies_only_include_intersect_ignored_files() {
    let fix = setup();
    let result = apply_worktree_include(&ApplyOptions {
        source_root: fix.main.clone(),
        target_root: fix.worktree.clone(),
        dry_run: false,
    })
    .expect("apply");

    assert!(result.include_file.is_some());
    let copied: HashSet<_> = result.copied.into_iter().collect();
    let expected: HashSet<_> = [".env", ".env.local", "config/secrets.json"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(copied, expected);
    assert_eq!(
        fs::read_to_string(fix.worktree.join(".env")).unwrap(),
        "SECRET=1\n"
    );
    assert_eq!(
        fs::read_to_string(fix.worktree.join("config/secrets.json")).unwrap(),
        "{\"k\":1}\n"
    );
    assert!(fix.worktree.join(".env.example").exists());
}

#[test]
fn skips_existing_and_dry_run_does_not_write() {
    let fix = setup();
    apply_worktree_include(&ApplyOptions {
        source_root: fix.main.clone(),
        target_root: fix.worktree.clone(),
        dry_run: false,
    })
    .unwrap();

    write(&fix.worktree.join(".env"), "ALREADY=1\n");
    let dry = apply_worktree_include(&ApplyOptions {
        source_root: fix.main.clone(),
        target_root: fix.worktree.clone(),
        dry_run: true,
    })
    .unwrap();
    assert!(dry.skipped_existing.iter().any(|p| p == ".env"));
    assert_eq!(
        fs::read_to_string(fix.worktree.join(".env")).unwrap(),
        "ALREADY=1\n"
    );

    let again = apply_worktree_include(&ApplyOptions {
        source_root: fix.main.clone(),
        target_root: fix.worktree.clone(),
        dry_run: false,
    })
    .unwrap();
    assert!(again.skipped_existing.iter().any(|p| p == ".env"));
    assert_eq!(
        fs::read_to_string(fix.worktree.join(".env")).unwrap(),
        "ALREADY=1\n"
    );
}

#[test]
fn noops_when_worktreeinclude_missing() {
    let tmp = tempfile::Builder::new()
        .prefix("herdr-wti-bare-")
        .tempdir()
        .unwrap();
    let bare = tmp.path().join("bare");
    let wt = tmp.path().join("bare-wt");
    fs::create_dir(&bare).unwrap();
    init_repo(&bare);
    write(&bare.join("a.txt"), "a\n");
    run_git(&["add", "a.txt"], &bare);
    run_git(&["commit", "-q", "-m", "init"], &bare);
    run_git(
        &[
            "worktree",
            "add",
            "-q",
            wt.to_str().unwrap(),
            "-b",
            "b2",
            "HEAD",
        ],
        &bare,
    );

    let result = apply_worktree_include(&ApplyOptions {
        source_root: bare,
        target_root: wt,
        dry_run: false,
    })
    .unwrap();
    assert!(result.include_file.is_none());
    assert!(result.copied.is_empty());
}
