# herdr-worktreeinclude

[![Herdr](https://img.shields.io/badge/herdr-plugin-4f46e5)](https://herdr.dev/plugins/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

A [Herdr](https://herdr.dev) plugin that restores selected **gitignored** local files into newly created Git worktrees, using the project’s `.worktreeinclude` file (same idea as [Claude Code worktrees](https://code.claude.com/docs/en/worktrees#copy-gitignored-files-into-worktrees)).

| | |
| --- | --- |
| Plugin id | `herdr-worktreeinclude` |
| Platforms | macOS, Linux |
| Min Herdr | `0.7.0` |
| Runtime | Native Rust binary (no Node/Python) |
| Build | `cargo` (at `plugin install`) |

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
herdr plugin install eightHundreds/herdr-worktreeinclude
```

Herdr runs `cargo build --release` during install and launches the resulting binary. You do **not** need Node, npm, Python, or TypeScript on the machine.

Requirements:

- [Herdr](https://herdr.dev) ≥ 0.7.0  
- [Rust toolchain](https://rustup.rs/) (`cargo` on `PATH`) for the install-time build  
- Git  

After install, only the compiled binary + Git are needed at runtime.

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
herdr plugin action invoke herdr-worktreeinclude.apply
```

Dry-run (from a built plugin tree, optional):

```bash
./target/release/herdr-worktreeinclude apply --dry-run
```

#### Logs

```bash
herdr plugin log list --plugin herdr-worktreeinclude
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

`herdr plugin link` does **not** run `[[build]]`. Build first:

```bash
git clone https://github.com/eightHundreds/herdr-worktreeinclude.git
cd herdr-worktreeinclude
cargo build --release
herdr plugin link "$(pwd)"
```

After code changes:

```bash
cargo build --release
# re-link if the manifest changed:
herdr plugin unlink herdr-worktreeinclude
herdr plugin link "$(pwd)"
```

Scripts:

```bash
cargo test
cargo build --release
```

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
| Install failed | Is `cargo` on PATH? See build logs during `plugin install` |
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
herdr plugin install eightHundreds/herdr-worktreeinclude
```

安装时 Herdr 会执行 `cargo build --release`，之后直接运行编译出的二进制。**不需要** Node、npm、Python 或 TypeScript。

机器需具备：

- [Herdr](https://herdr.dev) ≥ 0.7.0  
- [Rust 工具链](https://rustup.rs/)（安装时需要 `cargo` 在 `PATH` 中）  
- Git  

安装完成后，运行时只依赖编译好的二进制和 Git。

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
herdr plugin action invoke herdr-worktreeinclude.apply
```

仅预览（需已 build 的插件目录）：

```bash
./target/release/herdr-worktreeinclude apply --dry-run
```

#### 日志

```bash
herdr plugin log list --plugin herdr-worktreeinclude
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

`herdr plugin link` **不会**执行 `[[build]]`，需先自行编译：

```bash
git clone https://github.com/eightHundreds/herdr-worktreeinclude.git
cd herdr-worktreeinclude
cargo build --release
herdr plugin link "$(pwd)"
```

改代码后：

```bash
cargo build --release
# 若改了 manifest，可重新 link：
herdr plugin unlink herdr-worktreeinclude
herdr plugin link "$(pwd)"
```

命令：

```bash
cargo test
cargo build --release
```

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
| 安装失败 | `cargo` 是否在 PATH？看 `plugin install` 的 build 日志 |
| UI 建 worktree 未拷贝 | 用插件日志确认是否触发 `worktree.created`；可手动跑 **Apply**。部分 Herdr 版本 UI 路径可能不同 |
| 文件被跳过 | 目标已存在，或路径是 tracked |

### 相关链接

- [Herdr 插件文档](https://herdr.dev/docs/plugins/)  
- [Claude Code：`.worktreeinclude`](https://code.claude.com/docs/en/worktrees#copy-gitignored-files-into-worktrees)  
- [git-worktreeinclude CLI](https://github.com/satococoa/git-worktreeinclude)  

---

## License

MIT — see [LICENSE](./LICENSE) if present; otherwise default to MIT for this repository.
