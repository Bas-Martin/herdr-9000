# herdr


<p align="center">
  <img src="assets/logo.png" alt="herdr" width="100" />
</p>

<p align="center">
  <a href="https://herdr.dev">herdr.dev</a> · <a href="#install">install</a> · <a href="https://herdr.dev/docs/quick-start/">quick start</a> · <a href="https://herdr.dev/docs/">docs</a>
</p>

<p align="center">
  English · <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-666666?labelColor=333333" alt="Apache 2.0 license" /></a>
  <a href="https://github.com/herdrdev/herdr/releases"><img src="https://img.shields.io/github/downloads/herdrdev/herdr/total?labelColor=333333&color=666666" alt="total GitHub release downloads" /></a>
  <a href="https://github.com/herdrdev/herdr/stargazers"><img src="https://img.shields.io/github/stars/herdrdev/herdr?labelColor=333333&color=666666&logo=github" alt="GitHub stars" /></a>
  <a href="https://github.com/herdrdev/herdr/releases/latest"><img src="https://img.shields.io/github/v/release/herdrdev/herdr?label=release&labelColor=333333&color=666666" alt="latest stable release" /></a>
  <a href="https://formulae.brew.sh/formula/herdr"><img src="https://img.shields.io/homebrew/v/herdr?label=homebrew&labelColor=333333&color=666666" alt="Homebrew version" /></a>
  <a href="https://x.com/herdrdev"><img src="https://img.shields.io/badge/follow-%40herdrdev-000000?logo=x&logoColor=white" alt="follow @herdrdev on X" /></a>
</p>

---

https://github.com/user-attachments/assets/043ec09f-4bdd-41d5-aee0-8fda6b83e267

**the runtime your coding agents live on.**

- **always running** — herdr is a background server; the terminals live inside it. close the lid, drop the network, or restart the machine; agents keep working and sessions come back. reattach from any terminal, or over ssh.
- **never hunt for the stuck one** — every pane is marked working, blocked, or idle. when an agent stops and needs an answer, herdr says so.
- **agent-native** — agents drive herdr through the cli and socket api: they can spawn panes, prompt each other, and wait until another agent is genuinely blocked. [agent skill →](https://herdr.dev/docs/agent-skill/)
- **runs what you already run** — claude code, codex, cursor, opencode, grok and the rest. herdr doesn't wrap or replace them; it owns their terminals.
- **keyboard and mouse, both first-class** — tmux-style prefix keys *and* click, drag, split. pick per moment, not per tool.
- **plugins** — extend panes and workflows. [browse the marketplace →](https://herdr.dev/plugins/)
- **one rust binary, no electron** — runs in whatever terminal you already use.

---

## install

```bash
curl -fsSL https://herdr.dev/install.sh | sh
```

or `brew install herdr` · `mise use -g herdr` · windows: `powershell -ExecutionPolicy Bypass -c "irm https://herdr.dev/install.ps1 | iex"` · [endpoint-protected Windows](https://herdr.dev/docs/windows-beta/) · [binaries](https://github.com/herdrdev/herdr/releases)

### Herdr 9000 fork

Herdr 9000 keeps the upstream persistent terminal runtime and adds a
project-aware agent workflow:

- persistent project registrations;
- Git worktree-backed workspaces grouped with their parent repository;
- durable tasks with repository or worktree locations;
- reusable prompt, skill, and MCP resources;
- recurring project automations;
- tmux-native subagent panes;
- GitHub issue-to-task and task-to-PR workflows;
- a versioned socket API schema for clients and integrations.

The examples below use `herdr9000`. The installed binary also accepts the
normal `herdr` command name.

#### Install

Install the published fork package:

```bash
npm install --global github:Bas-Martin/herdr-9000
herdr9000
```

For the exact current branch, build from source:

```bash
git clone https://github.com/Bas-Martin/herdr-9000
cd herdr-9000
cargo build --release
```

Run `target/release/herdr` (or `target/release/herdr.exe` on Windows). The
published npm package may lag behind unreleased commits; a source build is the
authoritative way to run the current `main` branch.

#### 1. Register a project

Projects are persistent registrations keyed by repository root. Register a
repository once before using task-linked worktrees:

```bash
herdr9000 project create --name herdr9000 --path /path/to/herdr-9000 --open
herdr9000 project list
herdr9000 project get <project_id>
herdr9000 project open <project_id>
herdr9000 project rename <project_id> "Herdr 9000"
herdr9000 project delete <project_id>
```

`project create` returns the generated project ID in its JSON response. Use
that ID for task, resource, and automation commands. Creating the same root
again reports the existing registration instead of creating a duplicate.

In the TUI, open the repository workspace context menu and choose **Project
settings**. Project settings can define:

- a project-specific worktree root and base branch;
- the default agent provider;
- repository-relative preserve patterns for ignored files;
- project environment variables and lifecycle commands through the socket API.

The lifecycle fields are `prepare`, `setup`, `run`, and `teardown`. The first
three run in order for worktree tasks; `teardown` runs when the task finishes.
Use `herdr9000 api schema --json` for the complete `project.create` and
`project.update` payloads.

#### 2. Create and manage worktrees

Worktrees are regular Herdr workspaces with Git checkout provenance:

```bash
herdr9000 worktree list --cwd /path/to/herdr-9000
herdr9000 worktree create \
  --cwd /path/to/herdr-9000 \
  --branch feature/example \
  --base main \
  --focus
herdr9000 worktree open --path /path/to/worktree --focus
herdr9000 worktree remove --workspace <workspace_id>
```

If `--branch` does not exist, Herdr creates it from `--base` or `HEAD`.
Without `--path`, Herdr uses the configured worktree directory and creates a
path shaped like `<repo>/<branch-slug>`. `worktree remove` removes the checkout
with `git worktree remove`; it never deletes the Git branch. Add `--force` for
a dirty checkout when Git requires it.

For the client-shell flow, register the repository as a project first. Then
choose **New worktree** from the project workspace context menu. Enter a task
name, review the generated `feat/<slug>` branch and checkout path, and submit.
The new worktree is grouped with the parent workspace. **Open worktree...**
opens an existing checkout in the same group, while **Delete worktree
checkout...** removes a linked checkout.

Git ownership checks are enabled by default. Use `--trust-repository` for a
single verified command, or configure the worktree defaults in the TUI:

```toml
[worktrees]
directory = "~/.herdr/worktrees"
auto_trust_dirs = true
create_by_default = true
```

`create_by_default = true` makes a task without an explicit worktree path use
a new branch and worktree. Disable it when repository tasks should stay in the
main checkout.

#### 3. Create durable tasks

Tasks persist independently from the terminal pane and keep their project,
location, branch, workspace, agent, prompt, resources, and status:

```bash
herdr9000 task create \
  --project <project_id> \
  --name "Review authentication flow" \
  --location worktree \
  --provider codex \
  --model gpt-5 \
  --prompt "Inspect the auth flow and propose a safe fix."
herdr9000 task list --project <project_id>
herdr9000 task open <task_id>
herdr9000 task retry <task_id>
herdr9000 task close <task_id>
```

Use `--location repository` to keep the task in the project repository.
`--location worktree` provisions a checkout when `--worktree-path` is absent;
`--branch` can select the branch name. Project environment variables are
inherited and task `--env NAME=VALUE` values override them. `--workspace-id`,
`--tab-id`, and `--pane-id` attach a task to an existing runtime target.

Inspect and publish task changes without leaving the task workflow:

```bash
herdr9000 task diff <task_id> --unified
herdr9000 task read <task_id> --path src/main.rs
herdr9000 task write <task_id> --path notes/review.md --content "..."
herdr9000 task git <task_id> stage --path src/main.rs
herdr9000 task git <task_id> commit --message "fix: handle auth edge case"
herdr9000 task git <task_id> push
herdr9000 task git <task_id> pr \
  --title "fix: handle auth edge case" \
  --body "Describes the change" \
  --base main
```

The task browser is available from the TUI global menu. Selecting a task
opens its workspace, restores its runtime target when possible, and exposes
its status and location. A task with a configured provider can launch that
agent automatically; a task without one remains review-ready for manual work.

#### 4. Reuse prompts, skills, and MCP resources

Resources are persisted and scoped as `global`, `project`, or `task`:

```bash
herdr9000 resource create \
  --kind prompt \
  --name review-checklist \
  --scope project \
  --project <project_id> \
  --content "Check validation, error handling, and regression coverage."
herdr9000 resource list --project <project_id>
herdr9000 task resources <task_id> --resource <resource_id>
herdr9000 resource update <resource_id> --disable
herdr9000 resource delete <resource_id>
```

Use `--kind skill` for reusable instructions and `--kind mcp` for MCP
configuration JSON. A resource can be restricted to a provider with
`--provider`. Scope and provider checks happen before task execution; disabled,
foreign, malformed, or unsupported resources are rejected. Prompt and skill
content is injected into the task prompt. MCP resources currently require the
Claude provider.

The TUI global menu contains **resources** and lets you create, edit, enable,
disable, assign, and inspect resources without manually copying configuration
between tasks.

#### 5. Schedule recurring automations

An automation creates a new task for its project on a cron schedule:

```bash
herdr9000 automation create \
  --name weekday-review \
  --project <project_id> \
  --cron "0 9 * * 1-5" \
  --prompt "Review the latest changes and report regressions." \
  --provider codex \
  --workspace-mode worktree
herdr9000 automation list --project <project_id>
herdr9000 automation run-now <automation_id>
herdr9000 automation update <automation_id> --pause
herdr9000 automation update <automation_id> --resume
herdr9000 automation delete <automation_id>
```

Use `--workspace-mode repository` to run in the project checkout or
`worktree` to provision an isolated checkout for each run. `--disable` keeps
an automation configured but inactive; `--include-disabled` includes it in
list output.

#### 6. Work with GitHub issues

Search issues and create a durable Herdr task from one:

```bash
herdr9000 task github-search \
  --repo OWNER/REPOSITORY \
  --query "authentication" \
  --limit 20
herdr9000 task github-create \
  --repo OWNER/REPOSITORY \
  --number 123 \
  --project <project_id> \
  --location worktree \
  --branch issue/123-authentication
```

The created task retains the issue context and can use the normal task
diff/read/write/Git workflow. GitHub credentials and repository permissions
are supplied by the local GitHub tooling.

#### 7. Start agents in tmux panes

Start an agent in an existing Herdr shell pane:

```bash
herdr9000 agent start reviewer --kind codex --pane <pane_id>
```

Or let Herdr create a tmux pane:

```bash
herdr9000 agent start reviewer \
  --kind codex \
  --tmux \
  --target <source_pane_id> \
  --cwd /path/to/herdr-9000
```

Arguments after `--` are passed to the agent executable:

```bash
herdr9000 agent start reviewer --kind claude --tmux -- --model sonnet
```

Manage Herdr-owned tmux panes:

```bash
herdr9000 tmux list
herdr9000 tmux capture <pane_id> --lines 80
herdr9000 tmux send-keys <pane_id> enter
herdr9000 tmux focus <pane_id>
herdr9000 tmux kill <pane_id>
```

The tmux pane is tracked as a Herdr subagent, including provider, parent pane,
status, current path, and recent output. The TUI global menu exposes the same
pane list. `agent start --pane` requires an interactive shell prompt;
`agent start --tmux` creates the topology and starts the canonical provider
executable.

#### 8. Use the socket API

The CLI and TUI use the same local socket API. Inspect the schema bundled with
the installed binary:

```bash
herdr9000 api schema
herdr9000 api schema --json
herdr9000 api schema --output herdr-api.schema.json
herdr9000 api snapshot
```

New clients should advertise and probe optional methods such as
`project.*`, `worktree.*`, `task.*`, `automation.*`, `resource.*`, and
`tmux.*`. Missing optional methods, older servers, and unavailable endpoints
remain client-local failures; they must not disconnect compatible sessions.

The upstream Herdr documentation remains the reference for sessions,
workspaces, tabs, panes, integrations, plugins, remote attach, and keyboard
controls: [quick start](https://herdr.dev/docs/quick-start/) ·
[CLI reference](https://herdr.dev/docs/cli-reference/) ·
[socket API](https://herdr.dev/docs/socket-api/).

To test the current source branch before installing a published package:

```bash
git clone https://github.com/Bas-Martin/herdr-9000
cd herdr-9000
cargo build --release
```

Then start the built binary in the repository checkout. It uses the current
project registry, task, worktree, resource, automation, and tmux features.

## docs

everything lives at [herdr.dev/docs](https://herdr.dev/docs/): [quick start](https://herdr.dev/docs/quick-start/) · [concepts](https://herdr.dev/docs/concepts/) · [supported agents](https://herdr.dev/docs/agents/) · [keyboard](https://herdr.dev/docs/keyboard/) · [configuration](https://herdr.dev/docs/configuration/) · [session state](https://herdr.dev/docs/session-state/) · [remote](https://herdr.dev/docs/persistence-remote/) · [integrations](https://herdr.dev/docs/integrations/) · [plugins](https://herdr.dev/docs/plugins/) · [socket api](https://herdr.dev/docs/socket-api/)

## thanks

every past sponsor and backer is listed in [SPONSORS.md](./SPONSORS.md) — thank you 🐑

enterprise / partnership: hey@herdr.dev

## agent instructions

if you are an ai agent helping with this repository, read [`AGENTS.md`](./AGENTS.md) before making changes and read [`CONTRIBUTING.md`](./CONTRIBUTING.md) before opening issues or PRs.

## development

```bash
git clone https://github.com/herdrdev/herdr
cd herdr
cargo build --release

just test        # unit tests
just check       # formatting, tests, and maintenance checks
```

## license

Herdr is licensed under the [Apache License 2.0](LICENSE).
