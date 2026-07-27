import { copyFile, cp, lstat, mkdir, mkdtemp, rm } from "node:fs/promises";
import { readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve, sep } from "node:path";

import {
  gitMainWorktreeRoot,
  gitRevParseTopLevel,
  gitWorktreePaths,
  runGit,
  splitNull,
} from "./git.js";

export type ApplyOptions = {
  sourceRoot: string;
  targetRoot: string;
  /** When true, plan only — no filesystem writes. */
  dryRun?: boolean;
};

export type ApplyResult = {
  includeFile: string | null;
  matched: number;
  copied: string[];
  skippedExisting: string[];
  skippedTracked: string[];
  skippedMissing: string[];
  skippedOtherWorktree: string[];
  skippedUnsafe: string[];
};

const INCLUDE_NAME = ".worktreeinclude";

/**
 * Copy files that match `.worktreeinclude` AND are git-ignored from source
 * into target. Tracked files are never copied. Existing destinations are skipped.
 */
export async function applyWorktreeInclude(
  options: ApplyOptions,
): Promise<ApplyResult> {
  const sourceRoot = gitRevParseTopLevel(options.sourceRoot);
  const targetRoot = gitRevParseTopLevel(options.targetRoot);
  const includeFile = join(sourceRoot, INCLUDE_NAME);

  const empty: ApplyResult = {
    includeFile: null,
    matched: 0,
    copied: [],
    skippedExisting: [],
    skippedTracked: [],
    skippedMissing: [],
    skippedOtherWorktree: [],
    skippedUnsafe: [],
  };

  let includeContents: string;
  try {
    includeContents = readFileSync(includeFile, "utf8");
  } catch {
    return empty;
  }

  // Evaluate patterns in an empty repo so source .gitignore cannot override
  // .worktreeinclude negations.
  const patternRoot = await mkdtemp(join(tmpdir(), "herdr-wti-patterns-"));
  try {
    runGit(["init", "-q"], { cwd: patternRoot });
    writeFileSync(join(patternRoot, ".gitignore"), includeContents);

    const ignored = runGit(
      ["ls-files", "--others", "--ignored", "--exclude-standard", "-z"],
      { cwd: sourceRoot, check: false },
    );
    if (ignored.status !== 0) {
      throw new Error(
        `git ls-files failed in ${sourceRoot}: ${ignored.stderr || ignored.stdout}`,
      );
    }

    const matched = runGit(
      ["check-ignore", "--no-index", "--stdin", "-z"],
      {
        cwd: patternRoot,
        input: ignored.stdout,
        check: false,
      },
    );
    // check-ignore exits 1 when nothing matches; still fine.
    if (matched.status !== 0 && matched.status !== 1) {
      throw new Error(
        `git check-ignore failed: ${matched.stderr || matched.stdout}`,
      );
    }

    const candidates = splitNull(matched.stdout);
    const otherWorktrees = gitWorktreePaths(sourceRoot).filter(
      (p) => resolve(p) !== resolve(sourceRoot),
    );
    const tracked = new Set(
      splitNull(
        runGit(["ls-files", "-z"], { cwd: targetRoot, check: false }).stdout,
      ),
    );

    const result: ApplyResult = {
      includeFile,
      matched: candidates.length,
      copied: [],
      skippedExisting: [],
      skippedTracked: [],
      skippedMissing: [],
      skippedOtherWorktree: [],
      skippedUnsafe: [],
    };

    for (const relativePath of candidates) {
      if (!isSafeRelativePath(relativePath)) {
        result.skippedUnsafe.push(relativePath);
        continue;
      }

      const sourcePath = join(sourceRoot, relativePath);
      const destinationPath = join(targetRoot, relativePath);

      if (isUnderOtherWorktree(sourcePath, otherWorktrees)) {
        result.skippedOtherWorktree.push(relativePath);
        continue;
      }

      let sourceStat;
      try {
        sourceStat = await lstat(sourcePath);
      } catch {
        result.skippedMissing.push(relativePath);
        continue;
      }

      if (tracked.has(relativePath)) {
        result.skippedTracked.push(relativePath);
        continue;
      }

      try {
        await lstat(destinationPath);
        result.skippedExisting.push(relativePath);
        continue;
      } catch {
        // destination missing — ok to copy
      }

      if (options.dryRun) {
        result.copied.push(relativePath);
        continue;
      }

      await mkdir(dirname(destinationPath), { recursive: true });
      if (sourceStat.isDirectory() && !sourceStat.isSymbolicLink()) {
        await cp(sourcePath, destinationPath, {
          recursive: true,
          force: false,
          errorOnExist: true,
          preserveTimestamps: true,
        });
      } else if (sourceStat.isSymbolicLink()) {
        // copyFile preserves nothing special for symlinks; use cp for links too
        await cp(sourcePath, destinationPath, {
          recursive: false,
          force: false,
          errorOnExist: true,
          preserveTimestamps: true,
        });
      } else {
        await copyFile(sourcePath, destinationPath);
      }
      result.copied.push(relativePath);
    }

    return result;
  } finally {
    await rm(patternRoot, { recursive: true, force: true });
  }
}

/**
 * Resolve source for a new worktree: prefer explicit source, else main worktree.
 */
export function resolveSourceRoot(
  targetRoot: string,
  preferredSource?: string,
): string {
  if (preferredSource) {
    try {
      return gitRevParseTopLevel(preferredSource);
    } catch {
      // fall through
    }
  }
  const main = gitMainWorktreeRoot(targetRoot);
  if (!main) {
    throw new Error(`Could not resolve main worktree for ${targetRoot}`);
  }
  return main;
}

function isSafeRelativePath(relativePath: string): boolean {
  if (!relativePath || relativePath.startsWith("/") || relativePath.includes("\0")) {
    return false;
  }
  const normalized = relativePath.replaceAll("\\", "/");
  if (normalized === ".." || normalized.startsWith("../") || normalized.includes("/../")) {
    return false;
  }
  // Reject absolute-like Windows paths if they ever appear
  if (/^[a-zA-Z]:/.test(normalized)) return false;
  return true;
}

function isUnderOtherWorktree(sourcePath: string, otherWorktrees: string[]): boolean {
  const abs = resolve(sourcePath);
  for (const wt of otherWorktrees) {
    const root = resolve(wt);
    if (abs === root || abs.startsWith(root + sep)) return true;
  }
  return false;
}

