import { applyWorktreeInclude, resolveSourceRoot } from "./apply.js";
import { parseWorktreeCreatedPaths } from "./parse-event.js";

function log(message: string): void {
  console.error(`worktreeinclude: ${message}`);
}

async function main(): Promise<number> {
  const paths = parseWorktreeCreatedPaths(process.env.HERDR_PLUGIN_EVENT_JSON);
  if (!paths?.targetRoot) {
    log("no worktree path in HERDR_PLUGIN_EVENT_JSON; skip");
    return 0;
  }

  let sourceRoot: string;
  try {
    sourceRoot = resolveSourceRoot(paths.targetRoot, paths.sourceRoot || undefined);
  } catch (err) {
    log(`could not resolve source worktree: ${err instanceof Error ? err.message : err}`);
    return 0;
  }

  try {
    const result = await applyWorktreeInclude({
      sourceRoot,
      targetRoot: paths.targetRoot,
    });

    if (!result.includeFile) {
      return 0;
    }

    log(
      `${sourceRoot} → ${paths.targetRoot}: copied ${result.copied.length}, ` +
        `skipped existing ${result.skippedExisting.length}, ` +
        `tracked ${result.skippedTracked.length}, missing ${result.skippedMissing.length}`,
    );
    for (const p of result.copied) {
      log(`copied: ${p}`);
    }
    return 0;
  } catch (err) {
    log(`apply failed: ${err instanceof Error ? err.message : err}`);
    // Do not fail worktree creation hard for unexpected errors in v0.1
    return 0;
  }
}

const code = await main();
process.exit(code);
