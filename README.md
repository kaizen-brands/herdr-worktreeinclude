# herdr-worktreeinclude

[![Herdr](https://img.shields.io/badge/herdr-plugin-4f46e5)](https://herdr.dev/plugins/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

A [Herdr](https://herdr.dev) plugin that restores selected **gitignored** local files into newly created Git worktrees, using the project’s `.worktreeinclude` file (same idea as [Claude Code worktrees](https://code.claude.com/docs/en/worktrees#copy-gitignored-files-into-worktrees)). It also provides a Kaizen action that delegates worktree creation to Kaizen’s canonical worktree helper.

| | |
| --- | --- |
| Plugin id | `kaizen.herdr-worktreeinclude` |
| Platforms | macOS, Linux (x86_64 + arm64) |
| Min Herdr | `0.7.0` |
| Runtime | Prebuilt Rust binary (no Node/Python/cargo) |
| Install | Downloads binary from GitHub Releases |

---

## English

### What it does

Git worktrees only check out **tracked** files. Local secrets and machine-specific files (`.env`, `config/secrets.json`, …) stay in the main checkout and are missing from every new worktree.

This plugin listens for Herdr’s `worktree.created` event and, after a worktree is created:

1. Reads `.worktreeinclude` from the **source** (main) repository root  
2. Selects paths that match those patterns **and** are ignored by Git  
3. Copies them into the **new** worktree  

If the repo has no `.worktreeinclude`, the hook is a no-op.

### Install

```bash
herdr plugin install kaizen-brands/herdr-worktreeinclude
```

On install, Herdr runs `install-prebuilt.sh`, which downloads the matching binary from [GitHub Releases](https://github.com/kaizen-brands/herdr-worktreeinclude/releases) into `bin/`. **End users do not need Rust, Node, or Python** — only `curl`, `tar`, and Git.

Requirements:

- [Herdr](https://herdr.dev) ≥ 0.7.0  
- Git  
- `curl` + `tar` (standard on macOS/Linux)  

Prebuilt targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`.

Check that the plugin is registered:

```bash
herdr plugin list
```

### Configure `.worktreeinclude`

In the **target project** root (not this plugin repo), create `.worktreeinclude`. Syntax is the same as [`.gitignore`](https://git-scm.com/docs/gitignore):

```gitignore
# .worktreeinclude — patterns only; commit this file if the team shares the list
.env
.env.local
config/secrets.json
!.env.example
```

Typical pairing with `.gitignore`:

```gitignore
# .gitignore
.env
.env.local
config/secrets.json
```

A path is copied only when **both** are true:

| Condition | Required |
| --- | --- |
| Matches a pattern in `.worktreeinclude` | yes |
| Classified as **ignored** by Git in the source checkout | yes |
| Exists on disk in the source checkout | yes |
| Is **not** a tracked file | tracked files are never copied |
| Destination does not already exist | existing files are skipped (no overwrite) |

You can commit `.worktreeinclude` (patterns only). Keep secret **values** out of Git; they remain local ignored files that this plugin copies between worktrees.

### Usage

#### Automatic (default)

1. Install the plugin (above).  
2. Add `.worktreeinclude` and ensure listed files exist and are gitignored in the main checkout.  
3. Create a worktree in Herdr as usual (`New worktree` or `herdr worktree create ...`).  

After creation, matching files should appear in the new worktree.

#### Manual

Re-run copy for the focused workspace:

```bash
herdr plugin action invoke kaizen.herdr-worktreeinclude.apply
```

Dry-run (from an installed or locally built plugin tree, optional):

```bash
./bin/herdr-worktreeinclude apply --dry-run
```

#### Kaizen worktree action

Use the **Create Kaizen worktree** action from a Herdr workspace when the
checkout must follow Kaizen’s canonical contract. The action delegates to
`kaizen-worktree.sh`, so it creates the central-root path, harness lane,
`<slug>-<id8>` name, branch, claim, and environment files before opening the
checkout in Herdr. It never asks Herdr’s native creator to make an intermediate
non-canonical checkout.

The action defaults to the `automation` harness because Herdr is a transport,
not a coding harness. Set `KAIZEN_HERDR_HARNESS` when the worktree belongs to a
specific harness, and set `KAIZEN_HERDR_WORKTREE_SLUG` when the workspace label
is not a useful task slug.

For direct CLI use, the same operation is available as:

```bash
./bin/herdr-worktreeinclude create-kaizen-worktree \
  --harness codex --slug preview-e2e
```

#### Logs

```bash
herdr plugin log list --plugin kaizen.herdr-worktreeinclude
```

Example lines:

```text
worktreeinclude: /path/main → /path/wt: copied 2, skipped existing 0, tracked 0, missing 0
worktreeinclude: copied: .env
```

### Safety defaults

- Never overwrites existing destination paths  
- Never copies tracked files  
- Missing `.worktreeinclude` → success, no work  
- Hook failures prefer not to break worktree creation (errors go to plugin logs)  
- Rejects unsafe relative paths (`..`, absolute paths)  
- Skips files that live under **other** linked worktrees of the same repo  

### Development / local link

`herdr plugin link` does **not** run `[[build]]`. Put a binary in `bin/` first:

```bash
git clone https://github.com/kaizen-brands/herdr-worktreeinclude.git
cd herdr-worktreeinclude
git remote add upstream https://github.com/eightHundreds/herdr-worktreeinclude.git
cargo build --release
mkdir -p bin && cp target/release/herdr-worktreeinclude bin/
herdr plugin link "$(pwd)"
```

After code changes:

```bash
cargo build --release
cp target/release/herdr-worktreeinclude bin/
# re-link if the manifest changed:
herdr plugin unlink kaizen.herdr-worktreeinclude
herdr plugin link "$(pwd)"
```

Or download the release binary for this machine:

```bash
bash install-prebuilt.sh
```

Scripts:

```bash
cargo test
cargo build --release
```

### Releasing prebuilts

Push a version tag matching `herdr-plugin.toml` (e.g. `version = "0.3.0"` → tag `v0.3.0`). GitHub Actions builds and publishes release assets that `install-prebuilt.sh` downloads.

### How matching works

Patterns are evaluated with Git’s own ignore engine (not a hand-rolled globber):

1. Create a temporary empty git repo  
2. Copy `.worktreeinclude` into that repo as `.gitignore`  
3. List ignored untracked paths in the source:  
   `git ls-files --others --ignored --exclude-standard`  
4. Filter with `git check-ignore --no-index` against the temporary patterns  

Evaluating patterns in an empty repo avoids the source `.gitignore` overriding `.worktreeinclude` negations (e.g. `!` exceptions).

### Troubleshooting

| Symptom | What to check |
| --- | --- |
| Nothing is copied | Is `.worktreeinclude` in the **main** repo root? Are source files present and gitignored? |
| Plugin not running | `herdr plugin list` — installed/enabled in this Herdr session? |
| Install failed | Network / GitHub Releases reachable? See `plugin install` logs. Unsupported arch? |
| UI worktree, no copy | Confirm `worktree.created` fired via plugin logs; run the **Apply** action manually. Some Herdr UI paths may differ by version. |
| File skipped | Destination already exists, or path is tracked |

### Related

- [Herdr plugins](https://herdr.dev/docs/plugins/)  
- [Claude Code: `.worktreeinclude`](https://code.claude.com/docs/en/worktrees#copy-gitignored-files-into-worktrees)  
- [git-worktreeinclude CLI](https://github.com/satococoa/git-worktreeinclude)  

---

## 中文

### 做什么

Git worktree 只会检出 **已被跟踪（tracked）** 的文件。主仓库里被 ignore 的本地文件（如 `.env`、`config/secrets.json`）不会出现在新 worktree 中。

本插件监听 Herdr 的 `worktree.created` 事件，在创建 worktree 之后：

1. 读取**源仓库**（主 checkout）根目录的 `.worktreeinclude`  
2. 选出同时满足「匹配该文件模式」且「被 Git ignore」的路径  
3. 复制到**新** worktree  

若仓库没有 `.worktreeinclude`，钩子直接成功、不做任何事。

### 安装

```bash
herdr plugin install kaizen-brands/herdr-worktreeinclude
```

安装时 Herdr 会执行 `install-prebuilt.sh`，从 [GitHub Releases](https://github.com/kaizen-brands/herdr-worktreeinclude/releases) 下载对应平台的预编译二进制到 `bin/`。**用户不需要** Rust、Node 或 Python，只要有 `curl`、`tar` 和 Git。

机器需具备：

- [Herdr](https://herdr.dev) ≥ 0.7.0  
- Git  
- `curl` + `tar`（macOS/Linux 一般自带）  

预编译目标：`aarch64-apple-darwin`、`x86_64-apple-darwin`、`x86_64-unknown-linux-musl`、`aarch64-unknown-linux-musl`。

确认插件已注册：

```bash
herdr plugin list
```

### 配置 `.worktreeinclude`

在**业务项目**根目录（不是本插件仓库）创建 `.worktreeinclude`，语法与 [`.gitignore`](https://git-scm.com/docs/gitignore) 相同：

```gitignore
# .worktreeinclude — 只写路径模式；团队共享列表时可提交此文件
.env
.env.local
config/secrets.json
!.env.example
```

通常与 `.gitignore` 配合：

```gitignore
# .gitignore
.env
.env.local
config/secrets.json
```

只有同时满足下列条件才会被拷贝：

| 条件 | 是否必须 |
| --- | --- |
| 匹配 `.worktreeinclude` 中的模式 | 是 |
| 在源仓库中被 Git 判定为 **ignored** | 是 |
| 源仓库磁盘上真实存在 | 是 |
| 不是 tracked 文件 | tracked 永远不拷 |
| 目标路径尚不存在 | 已存在则跳过（不覆盖） |

`.worktreeinclude` 本身可以提交（只有模式）。密钥**内容**仍留在被 ignore 的本地文件里，由本插件在 worktree 之间拷贝。

### 使用

#### 自动（默认）

1. 安装插件（见上）。  
2. 在主仓库写好 `.worktreeinclude`，并保证所列文件存在且已被 git ignore。  
3. 像往常一样在 Herdr 中创建 worktree（侧边栏 **New worktree** 或 `herdr worktree create ...`）。  

创建完成后，匹配的文件应出现在新 worktree 中。

#### 手动

对当前焦点 workspace 再执行一次拷贝：

```bash
herdr plugin action invoke kaizen.herdr-worktreeinclude.apply
```

仅预览（需已安装或本地构建的插件目录）：

```bash
./bin/herdr-worktreeinclude apply --dry-run
```

#### Kaizen worktree 操作

在 Herdr workspace 中使用 **Create Kaizen worktree** 操作时，插件会调用
Kaizen 的规范 worktree helper，创建正确的中央路径、harness、`<slug>-<id8>`
名称、分支、claim 和环境文件，然后在 Herdr 中打开该 checkout。它不会先让
Herdr 创建一个不符合 Kaizen 规范的中间 checkout。

默认 harness 是 `automation`，因为 Herdr 是传输层而不是 coding harness。
如果任务属于特定 harness，可以设置 `KAIZEN_HERDR_HARNESS`；如果 workspace
标签不适合作为任务 slug，可以设置 `KAIZEN_HERDR_WORKTREE_SLUG`。

#### 日志

```bash
herdr plugin log list --plugin kaizen.herdr-worktreeinclude
```

示例：

```text
worktreeinclude: /path/main → /path/wt: copied 2, skipped existing 0, tracked 0, missing 0
worktreeinclude: copied: .env
```

### 安全默认行为

- 不覆盖目标已有文件  
- 不拷贝 tracked 文件  
- 无 `.worktreeinclude` → 成功且无操作  
- 钩子异常优先不阻断 worktree 创建（错误进插件日志）  
- 拒绝不安全相对路径（`..`、绝对路径等）  
- 跳过位于**其它**已关联 worktree 下的路径  

### 本地开发 / link

`herdr plugin link` **不会**执行 `[[build]]`，需先把二进制放到 `bin/`：

```bash
git clone https://github.com/kaizen-brands/herdr-worktreeinclude.git
cd herdr-worktreeinclude
git remote add upstream https://github.com/eightHundreds/herdr-worktreeinclude.git
cargo build --release
mkdir -p bin && cp target/release/herdr-worktreeinclude bin/
herdr plugin link "$(pwd)"
```

改代码后：

```bash
cargo build --release
cp target/release/herdr-worktreeinclude bin/
# 若改了 manifest，可重新 link：
herdr plugin unlink kaizen.herdr-worktreeinclude
herdr plugin link "$(pwd)"
```

或直接拉本机对应的 Release 二进制：

```bash
bash install-prebuilt.sh
```

命令：

```bash
cargo test
cargo build --release
```

### 发布预编译包

推送与 `herdr-plugin.toml` 版本一致的 tag（例如 `version = "0.3.0"` → `v0.3.0`）。GitHub Actions 会构建并发布 Release 资源，供 `install-prebuilt.sh` 下载。

### 匹配原理

使用 Git 自带的 ignore 引擎（不手写 glob 解析）：

1. 创建临时空 git 仓库  
2. 将 `.worktreeinclude` 复制为该仓库的 `.gitignore`  
3. 在源仓库列出 ignored 未跟踪路径：  
   `git ls-files --others --ignored --exclude-standard`  
4. 用 `git check-ignore --no-index` 按临时模式过滤  

在空仓库中求模式，可避免源仓库 `.gitignore` 覆盖 `.worktreeinclude` 的否定规则（如 `!` 例外）。

### 排查

| 现象 | 检查 |
| --- | --- |
| 什么都没拷 | 主仓库根是否有 `.worktreeinclude`？源文件是否存在且被 ignore？ |
| 钩子没跑 | `herdr plugin list` — 当前 Herdr session 是否已安装/启用？ |
| 安装失败 | 能否访问 GitHub Releases？看 `plugin install` 日志。架构是否支持？ |
| UI 建 worktree 未拷贝 | 用插件日志确认是否触发 `worktree.created`；可手动跑 **Apply**。部分 Herdr 版本 UI 路径可能不同 |
| 文件被跳过 | 目标已存在，或路径是 tracked |

### 相关链接

- [Herdr 插件文档](https://herdr.dev/docs/plugins/)  
- [Claude Code：`.worktreeinclude`](https://code.claude.com/docs/en/worktrees#copy-gitignored-files-into-worktrees)  
- [git-worktreeinclude CLI](https://github.com/satococoa/git-worktreeinclude)  

---

## License

MIT — see [LICENSE](./LICENSE) if present; otherwise default to MIT for this repository.
