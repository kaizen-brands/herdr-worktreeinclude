import { spawnSync } from "node:child_process";

export function runGit(
  args: string[],
  options: { cwd?: string; input?: string | Buffer; check?: boolean } = {},
): { status: number; stdout: string; stderr: string } {
  const result = spawnSync("git", args, {
    cwd: options.cwd,
    input: options.input,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });

  if (result.error) {
    throw result.error;
  }

  const status = result.status ?? 1;
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";

  if (options.check !== false && status !== 0) {
    throw new Error(
      `git ${args.join(" ")} failed (${status}): ${stderr || stdout}`.trim(),
    );
  }

  return { status, stdout, stderr };
}

export function gitRevParseTopLevel(cwd: string): string {
  return runGit(["rev-parse", "--show-toplevel"], { cwd }).stdout.trim();
}

/** First worktree path from `git worktree list --porcelain` (main checkout). */
export function gitMainWorktreeRoot(cwd: string): string | null {
  const { stdout, status } = runGit(["worktree", "list", "--porcelain"], {
    cwd,
    check: false,
  });
  if (status !== 0) return null;
  for (const line of stdout.split("\n")) {
    if (line.startsWith("worktree ")) {
      return line.slice("worktree ".length);
    }
  }
  return null;
}

/** All linked worktree absolute paths for a repository. */
export function gitWorktreePaths(cwd: string): string[] {
  const { stdout, status } = runGit(["worktree", "list", "--porcelain"], {
    cwd,
    check: false,
  });
  if (status !== 0) return [];
  const paths: string[] = [];
  for (const line of stdout.split("\n")) {
    if (line.startsWith("worktree ")) {
      paths.push(line.slice("worktree ".length));
    }
  }
  return paths;
}

/** Null-delimited paths from stdout. */
export function splitNull(stdout: string): string[] {
  if (!stdout) return [];
  return stdout.split("\0").filter((p) => p.length > 0);
}
