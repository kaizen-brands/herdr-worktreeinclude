import { applyWorktreeInclude, resolveSourceRoot } from "./apply.js";
import { gitRevParseTopLevel } from "./git.js";
import { parseWorktreeCreatedPaths } from "./parse-event.js";

/**
 * Manual action: apply .worktreeinclude into the focused worktree.
 * Prefer HERDR_PLUGIN_CONTEXT_JSON / cwd; fall back to event JSON if present.
 */
async function main(): Promise<number> {
  const dryRun = process.argv.includes("--dry-run");
  const targetRoot = resolveTargetFromEnv() ?? process.cwd();

  let sourceRoot: string;
  try {
    const event = parseWorktreeCreatedPaths(process.env.HERDR_PLUGIN_EVENT_JSON);
    sourceRoot = resolveSourceRoot(targetRoot, event?.sourceRoot || undefined);
  } catch (err) {
    console.error(
      `worktreeinclude: ${err instanceof Error ? err.message : err}`,
    );
    return 1;
  }

  const result = await applyWorktreeInclude({
    sourceRoot,
    targetRoot,
    dryRun,
  });

  if (!result.includeFile) {
    console.log(`No .worktreeinclude in ${sourceRoot}; nothing to do.`);
    return 0;
  }

  const verb = dryRun ? "would copy" : "copied";
  console.log(
    `${verb} ${result.copied.length} path(s); ` +
      `skipped existing=${result.skippedExisting.length} ` +
      `tracked=${result.skippedTracked.length} ` +
      `missing=${result.skippedMissing.length}`,
  );
  for (const p of result.copied) {
    console.log(`  ${verb}: ${p}`);
  }
  return 0;
}

function resolveTargetFromEnv(): string | null {
  const raw = process.env.HERDR_PLUGIN_CONTEXT_JSON;
  if (raw) {
    try {
      const ctx = JSON.parse(raw) as Record<string, unknown>;
      const candidates = [
        dig(ctx, ["worktree", "checkout_path"]),
        dig(ctx, ["worktree", "path"]),
        dig(ctx, ["workspace_cwd"]),
        dig(ctx, ["focused_pane_cwd"]),
      ];
      for (const c of candidates) {
        if (typeof c === "string" && c.length > 0) {
          try {
            return gitRevParseTopLevel(c);
          } catch {
            // try next
          }
        }
      }
    } catch {
      // ignore
    }
  }

  for (const key of ["HERDR_WORKSPACE_CWD", "PWD"] as const) {
    const v = process.env[key];
    if (v) {
      try {
        return gitRevParseTopLevel(v);
      } catch {
        // try next
      }
    }
  }
  return null;
}

function dig(obj: unknown, path: string[]): unknown {
  let cur: unknown = obj;
  for (const key of path) {
    if (!cur || typeof cur !== "object") return undefined;
    cur = (cur as Record<string, unknown>)[key];
  }
  return cur;
}

const code = await main();
process.exit(code);
