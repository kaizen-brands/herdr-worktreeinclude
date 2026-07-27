export type WorktreeCreatedPaths = {
  sourceRoot: string;
  targetRoot: string;
};

/**
 * Extract source (main) and target (new worktree) roots from HERDR_PLUGIN_EVENT_JSON.
 * Field names confirmed against herdr 0.7.x community payloads.
 */
export function parseWorktreeCreatedPaths(
  raw: string | undefined,
): WorktreeCreatedPaths | null {
  if (!raw?.trim()) return null;

  let event: unknown;
  try {
    event = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!event || typeof event !== "object") return null;

  const root = event as Record<string, unknown>;
  const data =
    root.data && typeof root.data === "object"
      ? (root.data as Record<string, unknown>)
      : root;

  const targetRoot = firstAbsolutePath([
    dig(data, ["worktree", "path"]),
    dig(root, ["worktree", "path"]),
    dig(data, ["worktree", "checkout_path"]),
  ]);

  const sourceRoot = firstAbsolutePath([
    dig(data, ["workspace", "worktree", "repo_root"]),
    dig(data, ["workspace", "worktree", "checkout_path"]),
    dig(data, ["source_worktree", "path"]),
  ]);

  if (!targetRoot) return null;
  if (!sourceRoot) return { sourceRoot: "", targetRoot };
  return { sourceRoot, targetRoot };
}

function dig(obj: unknown, path: string[]): unknown {
  let cur: unknown = obj;
  for (const key of path) {
    if (!cur || typeof cur !== "object") return undefined;
    cur = (cur as Record<string, unknown>)[key];
  }
  return cur;
}

function firstAbsolutePath(candidates: unknown[]): string | null {
  for (const c of candidates) {
    if (typeof c === "string" && c.startsWith("/")) return c;
  }
  return null;
}
