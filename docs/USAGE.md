# Korlap — Usage Guide

A walkthrough of how to use Korlap day-to-day: each kanban phase, what you can do in it, and how to move work to the next phase.

---

## What Korlap is

Korlap is a desktop orchestrator for parallel Claude Code agents. The core idea:

- **One task = one workspace = one git worktree on its own branch.**
- Each workspace runs an isolated `claude` subprocess with its own session, message history, and tool permissions.
- The kanban board is the lifecycle view: cards move from idea → spec → implementation → review → merge.

You stay in control of what runs. Korlap never spawns an agent without your intent (unless Autopilot is on, which is itself an explicit toggle).

---

## Setup

### Prerequisites

- [Bun](https://bun.sh/) — package manager + runtime.
- [Rust](https://rustup.rs/) — stable toolchain for the Tauri backend.
- [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code) — installed and authenticated.
- [GitHub CLI](https://cli.github.com/) (`gh`) — required for PR creation, recommended for multi-account workflows.
- (Optional) [openspec](https://github.com/derekchristensen/openspec) on `$PATH` if you want the Spec phase to auto-init proposals.

### First run

```bash
bun install
bun tauri dev
```

The app opens to a Home screen. From there:

1. **Bind a GitHub profile.** Korlap reads tokens via `gh auth token --user <profile>`; it never calls `gh auth switch`. Add as many profiles as you have orgs.
2. **Add a repo.** Folder picker → pick a local clone. Korlap binds the chosen `gh` profile to that repo.
3. **Open the repo.** Plan mode (`⌘1`) opens; you'll see an empty kanban.

### Where data lives

All Korlap state is stored in `~/Library/Application Support/net.ghora.korlap/`. The managed git repo is **never** written to (no dotfiles, no metadata, nothing). Worktrees go under `<data_dir>/workspaces/<workspace-id>/`.

---

## The two modes

Toggle with the title-bar buttons or shortcuts:

- **Plan mode (`⌘1`)** — kanban board. Where you create tasks, watch agent progress, and drag cards between phases.
- **Work mode (`⌘2`)** — single-workspace view: chat, diff, files, terminal, scripts. You enter Work mode by clicking a card on the kanban (⌘-click on a card jumps straight to chat).

Plan mode also has sub-views switchable via the top tabs: Kanban (default), Files (cross-workspace file browser), Terminal (repo-level shell tabs).

---

## The five phases

```
┌────────┐    ┌────────┐    ┌─────────────┐    ┌────────┐    ┌────────┐
│  Todo  │ →  │  Plan  │ →  │ In Progress │ →  │ Review │ →  │  Done  │
└────────┘    └────────┘    └─────────────┘    └────────┘    └────────┘
     │             │              ↑                ↑              ↑
     └─────────────┴──────────────┘                │              │
              (or skip Plan)                       │              │
                                                  push          merge PR
                                            (Create PR is a
                                             separate action)
```

Note: the **column header reads "Plan"**, but the underlying phase value is `"spec"` in code and storage. They're the same thing — the UI calls it Plan because the agent is planning the change.

---

### Phase 1 — Todo

**What it represents.** A task you've defined but no agent is working on yet. No worktree, no branch, no process. Just a card.

**What you can do.**

- **New task** (`⌘N` on Plan mode, or "+ New task" button). Opens TaskPopover. You can set:
  - Title, description.
  - Plan-mode toggle (Claude `--permission-mode plan` for the first turn).
  - Thinking-mode toggle.
  - Model override (Sonnet / Opus / Haiku).
  - Pasted images (referenced from disk; not embedded).
  - `@file` mentions (referenced files inserted as paths the agent will read).
  - Defaults: title-bar Plan-mode toggle, repo settings (`default_thinking`, `default_plan`).
- **Add and Start.** Submit-and-spawn shortcut. Skips the staging step; goes straight to whatever `default_start_phase` is configured to.
- **Edit / Remove.** Pencil and × buttons on the card.
- **Mark Ready.** Toggle on the card. Used by Autopilot to know which todos are eligible for auto-spawn.
- **Reorder.** Drag a Todo card up/down within the Todo column.
- **Bulk import from Jira.** Plan column footer → "..." menu → Import from Jira. Filters by Jira status, creates one Todo card per ticket.

**How to enter Todo.**

- Click "New task" (⌘N).
- Submit a TaskPopover.
- Import from Jira.

**How to leave Todo.**

- **Drag onto Plan** → spawns a workspace in Spec phase. Plan mode auto-on.
- **Drag onto In Progress** → spawns a workspace in implementing phase. Skips planning entirely.
- **Click Start (▶)** → uses the repo's `default_start_phase` setting (Spec or Implementing).
- **Autopilot** can auto-spawn from any Todo marked Ready (see Autopilot below).

---

### Phase 2 — Plan (internal: `spec`)

**What it represents.** A workspace whose job is to design the change, not implement it. The worktree exists, the agent is alive, but the agent runs in plan mode and writes to an `openspec` proposal (if `openspec_enabled`).

**What you can do.**

- All workspace tabs are active: Chat, Diff, Files, Terminal, Scripts. The Diff is normally empty during planning — the agent shouldn't be writing code.
- **Chat** with the agent to refine the spec, push back on the approach, ask questions about the codebase.
- **Plan mode is auto-enabled** for the first turn. You can flip it off mid-conversation, but the column expects spec output, not code edits.
- If `openspec_enabled` is on:
  - The worktree gets `openspec init` run on creation.
  - The first prompt the agent receives is `/opsx:propose\n\nUse proposal name: <slug>\n\n<your task>`. The slug is derived from the ticket title (lowercase, kebab-case, ASCII-only, e.g. `Add user login flow` → `add-user-login-flow`).
- **All cross-cutting features** (LSP, MCP, knowledge base, mentions, images, scripts) are available.

**How to enter Plan.**

- Drag a Todo card onto the Plan column.
- Click Start on a Todo when `default_start_phase = spec`.
- Submit a TaskPopover with "Add and Start" when `default_start_phase = spec`.

**How to leave Plan.**

- **Drag onto In Progress.** This is the explicit "I'm done planning" gesture. It:
  1. Calls `set_workspace_phase(wsId, "implementing")` on the backend.
  2. Disables plan mode for subsequent turns.
  3. Sends a follow-up prompt: `/opsx:apply\n\nApply proposal: <slug>\n\nProceed with implementation of the plan above.` (the `/opsx:apply` lines only when `openspec_enabled`).
- **Drag back onto Plan or Todo is not allowed.** Workspaces don't reverse phase.
- **Remove the workspace** from the card menu — destroys the worktree and the agent. Use this when you've decided the task isn't worth doing.

---

### Phase 3 — In Progress

**What it represents.** A workspace where the agent is actively implementing. Full tool permissions (`bypassPermissions`), no plan-mode constraint.

**What you can do.**

Per-workspace, all tabs:

- **Chat** — type prompts, queue messages while the agent is busy, paste images, mention `@file` paths. Slash commands work (e.g. `/caveman ultra`).
- **Diff** — unified diff against the merge-base. Syntax-highlighted. Click a hunk header to copy it back into the chat as context.
- **Files** — browse the worktree as a tree. CodeMirror 6 editor for in-place edits (use sparingly — the agent owns the worktree).
- **Terminal** — raw PTY tabs. Each tab is a separate `zsh` in the worktree directory. Multiple tabs per workspace.
- **Scripts** — buttons for the named `run_scripts` configured in repo settings, executed inside the worktree with envs `KORLAP_WORKSPACE_NAME`, `KORLAP_WORKSPACE_PATH`, `KORLAP_ROOT_PATH`, `KORLAP_DEFAULT_BRANCH`.

Top-bar workspace actions (Work mode):

- **Review (`⌘R`)** — AI self-review of the current diff. Streams findings into a Review pill at the top of the chat. **This does not move the card to Review** — it's a critique, not a status change. After completion, "Send to chat" injects the report into the conversation so you can ask the agent to address it.
- **Push (`⌘P`)** — commits any local changes (auto-generated message) and pushes the branch to origin. Does **not** open a PR. The button label flips to "Commit & push" when there are uncommitted changes, "Push" when there are unpushed commits, and is disabled once everything is pushed.
- **Create PR (`⌘M`)** — asks the agent to run `gh pr create --base <default>` and opens a pull request. Disabled until the branch is fully pushed (no uncommitted changes, no unpushed commits) — push first, then create the PR.
- **Update branch from base** — visible when your branch is behind the default branch. Runs `git merge origin/<base>` inside the worktree; if conflicts arise, the conflict is auto-delegated to the agent.
- **Plan / Thinking / Model toggles** in the chat header — per-workspace overrides on top of the repo defaults.
- **Provider switch** (Claude ↔ Codex) — per-workspace.
- **Rename workspace / branch.** Double-click the sidebar entry.
- **Remove workspace.** Sidebar `…` menu or row hover. Removes the worktree and kills the agent.

**How to enter In Progress.**

- Drag a Todo card directly onto In Progress (skip planning).
- Drag a Plan workspace onto In Progress (advance from spec).
- Click Start on a Todo when `default_start_phase = implementing`.
- Manual checkout (Plan column footer → "..." → Manual checkout) — start a workspace from any local branch.
- PR checkout (More menu → Review PR) — checkout an existing PR's branch into a workspace.
- Combo PR checkout (More menu → Combo PRs) — merge multiple PR branches into one synthetic workspace for integration testing.

**How to leave In Progress.**

- **Drag onto Review** → opens a confirm dialog: "Commit local changes (if any) and push `<branch>` to origin". On confirm, runs `triggerPushAction(wsId)` — the branch is committed and pushed but **no PR is opened**. The card moves to Review only after a PR exists for the branch (which still requires a manual **Create PR** click in Work mode, or the agent opening one on its own). Drag-to-Review is now a "ship the code" gesture, not a "ship the code AND open the PR" gesture.
- **Open the PR yourself** — switch to Work mode and click **Create PR** (`⌘M`). The button is enabled once the branch is fully pushed.
- The card can also move to Review automatically if the agent itself creates a PR (e.g. the `pr_message` workflow finishes without you dragging anything).
- **Remove workspace** — destroys the worktree and agent.

---

### Phase 4 — Review

**What it represents.** A workspace whose branch has an open PR on GitHub. The implementation work is done; you're waiting on review feedback or your own go-ahead to merge.

**What you can do.**

- Everything from In Progress (chat, diff, files, terminal, scripts) still works. You can keep iterating — every push updates the PR.
- **AI Review (`⌘R`)** is still available; useful for a final pass before merging.
- **PR status pill** in the workspace header — shows PR number, state, link to GitHub.
- **Branch sync** — keep merging the base branch in if it advances.
- **Rebase / force-push** is the agent's job; don't `git push --force` directly from the terminal unless you know what you're doing.

**How to enter Review.**

- Drag from In Progress (pushes the branch — does **not** open a PR by itself; the card stays in In Progress until a PR exists).
- Click **Create PR** in Work mode after pushing.
- Open an existing PR via PR checkout.
- The card moves automatically once a PR opens for the workspace's branch (whether you triggered Create PR, the agent opened one on its own, or autopilot did).

**How to leave Review.**

- **Drag onto Done** → confirm dialog: "This will merge PR #N (`label`) into `<base>`. This cannot be undone." On confirm, runs the PR merge workflow.
- **Card moves to Done automatically when the PR merges** (regardless of whether you triggered it from Korlap or directly on GitHub).
- **PR closed without merge.** The card relocates back to In Progress (no longer matches the open-PR filter).

---

### Phase 5 — Done

**What it represents.** A workspace whose PR has been merged. Read-only by convention; the worktree is still on disk so you can audit the final diff.

**What you can do.**

- Open the workspace and inspect the chat/diff/files for posterity.
- **Remove individual** — the row × button.
- **Remove all** — the "..." menu on the Done column header → "Remove all". Wipes every Done workspace's worktree.
- **Knowledge base update** runs on merge if enabled (invariants/facts re-derived from the merged commit).

**How to enter Done.**

- Drag from Review (merge flow).
- The card relocates automatically when the PR is merged.

**How to leave Done.**

- Done is terminal. Workspaces in Done can only be removed.

---

## Cross-cutting capabilities

These are not phase-specific — they're available wherever they make sense.

### Autopilot

Toggle in the title bar (look for the autopilot pill at the bottom-right of the kanban). Autopilot:

- Spawns workspaces from Todos marked **Ready**, up to `max_agents` parallel (default 3). Phase respects `default_start_phase`.
- Runs **AI Review** on idle In Progress workspaces with diffs (capped at 5 review cycles per workspace).
- **Auto-creates PRs** when a workspace's review reports clean.
- **Auto-resolves merge conflicts** by sending the conflict context to the agent.
- Streams events into the autopilot pill. You can type a free-form command into the pill (e.g. "skip ws-X" or "prioritize Y") — that goes through the orchestrator.

Autopilot blacklists itself: branches like `feat/autopilot-mode` are never auto-spawned.

### LSP

Per-repo language server pool. Configurable in **Settings → LSP**. Korlap ships built-in defaults for common languages (Rust, TypeScript, Svelte, Go, etc.) and you can add custom servers. LSP services are exposed to the agent via the built-in MCP server, so the agent can do hover, go-to-definition, find-references, rename, and read diagnostics across the worktree.

### MCP

Two layers:

- **Built-in `korlap` MCP server.** Always running on a random port. Exposes workspace state, files, LSP, search, and grep tools to every agent.
- **User-configured MCP servers.** Settings → MCP. Add stdio or SSE servers (Jira, Slack, custom APIs). They're merged with the built-in server when the agent starts.

### Knowledge base

Per-repo. Settings → Knowledge. Build a structured context (invariants + facts + invariants + contradictions) from the codebase using a precheck model. Re-built incrementally on PR merge. The agent can query the knowledge base via MCP and receives invariant violations as part of the AI Review flow.

### AI Review (`⌘R`)

Self-critique pass on a workspace's diff. Uses the configured `review_message` template plus `DEFAULT_REVIEW_PROMPT`, with substitutions for `{{branch}}`, `{{base_branch}}`, `{{pr_number}}`, `{{pr_title}}`. Output streams into a Review pill; the pill exposes "Send to chat" so you can ask the agent to address findings.

### Branch sync

When the workspace's base branch advances, the workspace header shows the count of commits behind. One click runs the merge in the worktree; conflicts auto-delegate to the agent.

### Combo workspaces

Plan column "..." → Combo PRs. Pick 2+ open PRs; Korlap creates a single workspace with all branches merged together for integration testing. Useful when feature branches need to be tested in combination before any one of them lands.

### Manual / PR / Jira import

- **Manual checkout** — start a workspace from any local branch (handy for revisiting old work).
- **PR checkout** — checkout an existing PR's branch into a fresh workspace; the agent gets context that this is a code-review session.
- **Jira import** — bulk-create Todos from a Jira filter.

---

## Repo settings reference

`⌘,` opens settings for the active repo. Tabs:

### Scripts

- **`setup_script`** — runs once when a new workspace is created. Useful for `bun install`, `cargo build`, etc.
- **`run_scripts`** — named commands surfaced as buttons in the Scripts tab. Each gets the `KORLAP_*` env vars.
- **`remove_script`** — runs before workspace deletion. Cleanup hook.
- **`pr_message`** — template injected into the prompt when the agent is asked to create a PR. Supports `{{file_count}}`, `{{branch}}`, `{{base_branch}}`.
- **`review_message`** — extra instructions appended to the AI Review prompt.

### Agent

- **`agent_provider`** — Claude or Codex default for new workspaces.
- **`system_prompt`** — prepended to every agent invocation in this repo.
- **`default_thinking`** — enable thinking mode for new workspaces / new chats.
- **`default_plan`** — enable Claude `--permission-mode plan` by default. Also seeds new Todos' plan-mode toggle.
- **`caveman_ultra`** — prepend `/caveman ultra` to every prompt. Cuts token usage ~75% with terse style.
- **`openspec_enabled`** — turns on the Spec-phase workflow: openspec init, `/opsx:propose`, `/opsx:apply`.
- **`default_start_phase`** — `spec` or `implementing`. Controls the Start button, "Add and Start", and Autopilot auto-pickup.

### LSP

User-configurable language servers. Status indicator per server. Restart / stop / reset to default per server.

### MCP

User-configured MCP servers (stdio or SSE). The built-in `korlap` server is implicit and not configurable here.

### Knowledge

Include / exclude globs, precheck model, build status. Buttons: build, incremental update, view contradictions.

### Appearance

Theme (Amber / Indigo / Peach / Slate). Color mode (Light / Dark / System).

---

## Keyboard shortcuts

| Shortcut | Where | Action |
|----------|-------|--------|
| `⌘1` | Global | Switch to Plan mode |
| `⌘2` | Global | Switch to Work mode |
| `⌘E` | Global | Open repo dropdown |
| `⌘,` | Global | Open repo settings |
| `⌘N` | Plan mode | New task |
| `⌘R` | Work mode | AI Review |
| `⌘P` | Work mode | Push (commit + push, no PR) |
| `⌘M` | Work mode | Create PR (enabled once branch is fully pushed) |
| `⌘U` | Work mode | Update branch from base |
| `⌘W` | Work mode | Remove workspace |
| `⌘⇧F` | Work mode | Search files |
| Arrow keys | Plan mode | Navigate kanban cards |
| `Enter` | Plan mode | Open focused card (⌘-Enter jumps into chat) |
| `Esc` | Plan mode | Clear focus |

Some shortcuts vary; if a shortcut doesn't fire, hover the relevant button — the tooltip shows the current binding.

---

## Common workflows

### A) Spec-first workflow

When you want every task to start with a written plan before any code is written.

1. Enable `openspec_enabled` in repo settings (and have `openspec` on `$PATH`).
2. Set `default_start_phase = spec`.
3. (Optional) Set `default_plan = true` so Claude plan-mode is the default for new chats too.
4. Add a Todo with title and short description. Click Start.
5. The workspace opens in the Plan column. Plan mode is on. The first prompt is `/opsx:propose ... Use proposal name: <slug>`. Chat with the agent until the spec satisfies you.
6. Drag the workspace onto In Progress. The agent receives `/opsx:apply ... Apply proposal: <slug>` and starts implementing.
7. When the diff is ready, drag onto Review (pushes the branch). Then click **Create PR** in Work mode to open the pull request — or let the agent do it.
8. After CI / human review, drag onto Done. Merge.

### B) Skip planning, go fast

For trivial fixes or follow-ups where a spec would be overhead.

1. Set `default_start_phase = implementing` (or just don't enable openspec).
2. Add a Todo. Drag it directly onto In Progress (this gesture always skips Plan).
3. Iterate in chat / diff / terminal until the change is done.
4. Drag to Review when ready.

### C) Review an external PR

When you want to use Korlap to read someone else's PR.

1. Plan column footer → "..." → Review PR.
2. Pick the PR from the list. Korlap creates a workspace with the PR's branch checked out and `source_pr` metadata attached.
3. Open the workspace; the diff and chat are scoped to the PR. Use `⌘R` for AI critique.
4. Comment on GitHub from the workspace header, or close the workspace when you're done — it doesn't push anything unless you ask.

---

## What Korlap deliberately doesn't do

- **No multi-repo simultaneous workspaces.** One repo at a time, by design.
- **No checkpoint / restore of Claude conversation history.** If you want to fork a thread, copy-paste.
- **No Windows support.**
- **No automatic agent spawning** unless Autopilot is on.
- **No writes to the managed repo** — every byte of Korlap state lives in `~/Library/Application Support/net.ghora.korlap/`.

---

## Troubleshooting

- **"Workspace stuck on Creating..."** — check `gh auth status` and that the repo's bound `gh` profile is still valid.
- **PR drag does nothing** — open the workspace and look for a permissions error in chat (`gh` token rejected, branch protected, etc.). The drag triggers a confirm dialog; if you cancel it the card stays put.
- **Spec workspace's first message has no `/opsx:propose`** — `openspec_enabled` is off, or the worktree's `openspec init` failed (check the workspace terminal for the warning).
- **Autopilot won't spawn** — make sure todos are marked Ready and the active-agent count is below `max_agents`.
- **LSP says "no server"** — check Settings → LSP for the language; the auto-detect uses `detect_files` patterns and may need a custom config.

For deeper hacking, see `CLAUDE.md` (architecture invariants) and `design.md` (design rationale).
