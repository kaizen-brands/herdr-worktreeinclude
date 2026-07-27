import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { parseWorktreeCreatedPaths } from "../src/parse-event.js";

describe("parseWorktreeCreatedPaths", () => {
  it("reads herdr 0.7 nested payload", () => {
    const raw = JSON.stringify({
      event: "worktree_created",
      data: {
        workspace: {
          workspace_id: "w1",
          worktree: {
            repo_root: "/repo/main",
            checkout_path: "/repo/main",
          },
        },
        worktree: {
          path: "/repo/worktrees/feature",
          branch: "feature",
        },
      },
    });
    assert.deepEqual(parseWorktreeCreatedPaths(raw), {
      sourceRoot: "/repo/main",
      targetRoot: "/repo/worktrees/feature",
    });
  });

  it("returns null for empty or invalid JSON", () => {
    assert.equal(parseWorktreeCreatedPaths(undefined), null);
    assert.equal(parseWorktreeCreatedPaths(""), null);
    assert.equal(parseWorktreeCreatedPaths("{"), null);
  });

  it("accepts target-only when source is missing", () => {
    const raw = JSON.stringify({
      data: { worktree: { path: "/tmp/wt" } },
    });
    assert.deepEqual(parseWorktreeCreatedPaths(raw), {
      sourceRoot: "",
      targetRoot: "/tmp/wt",
    });
  });
});
