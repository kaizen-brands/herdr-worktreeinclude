import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
  existsSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";

import { applyWorktreeInclude } from "../src/apply.js";
import { runGit } from "../src/git.js";

function initRepo(dir: string): void {
  runGit(["init", "-q"], { cwd: dir });
  runGit(["config", "user.email", "test@example.com"], { cwd: dir });
  runGit(["config", "user.name", "test"], { cwd: dir });
}

describe("applyWorktreeInclude", () => {
  let root: string;
  let main: string;
  let worktree: string;

  before(() => {
    root = mkdtempSync(join(tmpdir(), "herdr-wti-test-"));
    main = join(root, "main");
    worktree = join(root, "feature");
    mkdirSync(main);

    initRepo(main);
    writeFileSync(join(main, "README.md"), "hello\n");
    writeFileSync(join(main, ".gitignore"), ".env\n.env.local\nconfig/secrets.json\n");
    writeFileSync(join(main, ".env"), "SECRET=1\n");
    writeFileSync(join(main, ".env.local"), "LOCAL=1\n");
    writeFileSync(join(main, ".env.example"), "EXAMPLE=1\n");
    mkdirSync(join(main, "config"));
    writeFileSync(join(main, "config", "secrets.json"), '{"k":1}\n');
    writeFileSync(join(main, "config", "app.json"), '{"app":true}\n');
    writeFileSync(
      join(main, ".worktreeinclude"),
      [".env", ".env.local", "config/secrets.json", "!.env.example", ""].join(
        "\n",
      ),
    );

    runGit(["add", "README.md", ".gitignore", ".env.example", "config/app.json", ".worktreeinclude"], {
      cwd: main,
    });
    // .env.example is tracked; .env is ignored
    runGit(["add", "-f", ".env.example"], { cwd: main, check: false });
    runGit(
      ["commit", "-q", "-m", "init"],
      { cwd: main },
    );

    runGit(["branch", "feature"], { cwd: main });
    runGit(["worktree", "add", "-q", worktree, "feature"], { cwd: main });
  });

  after(() => {
    try {
      runGit(["worktree", "remove", "--force", worktree], {
        cwd: main,
        check: false,
      });
    } catch {
      // ignore
    }
    rmSync(root, { recursive: true, force: true });
  });

  it("copies only include∩ignored files", async () => {
    const result = await applyWorktreeInclude({
      sourceRoot: main,
      targetRoot: worktree,
    });

    assert.ok(result.includeFile);
    assert.deepEqual(new Set(result.copied), new Set([
      ".env",
      ".env.local",
      "config/secrets.json",
    ]));
    assert.equal(readFileSync(join(worktree, ".env"), "utf8"), "SECRET=1\n");
    assert.equal(
      readFileSync(join(worktree, "config/secrets.json"), "utf8"),
      '{"k":1}\n',
    );
    // tracked example should not be re-copied from include negation path
    assert.ok(existsSync(join(worktree, ".env.example")));
  });

  it("skips existing destinations and dry-run does not write", async () => {
    writeFileSync(join(worktree, ".env"), "ALREADY=1\n");
    const dry = await applyWorktreeInclude({
      sourceRoot: main,
      targetRoot: worktree,
      dryRun: true,
    });
    assert.ok(dry.skippedExisting.includes(".env"));
    assert.equal(readFileSync(join(worktree, ".env"), "utf8"), "ALREADY=1\n");

    const again = await applyWorktreeInclude({
      sourceRoot: main,
      targetRoot: worktree,
    });
    assert.ok(again.skippedExisting.includes(".env"));
    assert.equal(readFileSync(join(worktree, ".env"), "utf8"), "ALREADY=1\n");
  });

  it("no-ops when .worktreeinclude is missing", async () => {
    const bare = mkdtempSync(join(root, "bare-"));
    initRepo(bare);
    writeFileSync(join(bare, "a.txt"), "a\n");
    runGit(["add", "a.txt"], { cwd: bare });
    runGit(["commit", "-q", "-m", "init"], { cwd: bare });
    const wt = join(root, "bare-wt");
    runGit(["worktree", "add", "-q", wt, "-b", "b2", "HEAD"], { cwd: bare });

    const result = await applyWorktreeInclude({
      sourceRoot: bare,
      targetRoot: wt,
    });
    assert.equal(result.includeFile, null);
    assert.equal(result.copied.length, 0);
  });
});
