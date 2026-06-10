---
marp: true
paginate: true
size: 16:9
title: Korlap — Feature Tour
author: Korlap team
style: |
  @import url('https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap');

  :root {
    --bg-base: #1a1613;
    --bg-card: #221d18;
    --border: #3a322a;
    --text: #e8e0d5;
    --text-dim: #c0b4a4;
    --accent: #e0a458;
    --accent-soft: #c98a3f;
    --ok: #9bb05a;
  }

  section {
    background: var(--bg-base);
    color: var(--text);
    font-family: 'Space Grotesk', system-ui, sans-serif;
    font-size: 26px;
    line-height: 1.5;
    padding: 60px 70px;
  }

  h1, h2, h3 {
    font-family: 'Space Grotesk', sans-serif;
    color: var(--text);
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  h1 { font-size: 52px; color: var(--accent); }
  h2 { font-size: 38px; border-bottom: 2px solid var(--border); padding-bottom: 12px; }
  h3 { font-size: 28px; color: var(--accent-soft); }

  a { color: var(--accent); }
  strong { color: var(--accent); font-weight: 600; }
  em { color: var(--text-dim); font-style: italic; }

  code {
    font-family: 'JetBrains Mono', monospace;
    background: var(--bg-card);
    color: var(--accent);
    padding: 2px 7px;
    border-radius: 5px;
    font-size: 0.85em;
  }
  pre {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 18px 22px;
  }
  pre code { background: transparent; color: var(--text); padding: 0; }

  table {
    border-collapse: separate;
    border-spacing: 0;
    font-size: 0.82em;
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
  }
  thead th {
    background: var(--accent);
    color: #1a1613;
    text-align: left;
    padding: 11px 16px;
    font-weight: 700;
    border: none;
  }
  tbody td {
    background: var(--bg-card);
    color: var(--text);
    padding: 11px 16px;
    border-top: 1px solid var(--border);
    vertical-align: top;
  }
  tbody tr:nth-child(even) td { background: var(--bg-base); }
  td strong, th strong { color: var(--accent); }

  blockquote {
    border-left: 4px solid var(--accent);
    color: var(--text-dim);
    padding-left: 20px;
    font-style: italic;
  }

  ul, ol { margin-top: 0.2em; }
  li { margin-bottom: 0.35em; }

  section::after {
    color: var(--text-dim);
    font-family: 'JetBrains Mono', monospace;
    font-size: 16px;
  }

  section.lead {
    justify-content: center;
    text-align: center;
  }
  section.lead h1 { font-size: 76px; }
  section.lead p { color: var(--text-dim); font-size: 30px; }

  .cols { display: flex; gap: 40px; }
  .cols > * { flex: 1; }
  .how {
    color: var(--text-dim);
    font-size: 0.8em;
    border-top: 1px solid var(--border);
    padding-top: 12px;
    margin-top: 14px;
  }
  .tag {
    display: inline-block;
    background: var(--bg-card);
    border: 1px solid var(--border);
    color: var(--accent);
    border-radius: 6px;
    padding: 3px 12px;
    font-size: 0.7em;
    font-family: 'JetBrains Mono', monospace;
  }

  section.shot { justify-content: flex-start; align-items: center; text-align: center; padding: 40px; }
  section.shot h2 { border: none; color: var(--accent); font-size: 28px; margin: 0 0 22px; padding: 0; }
  section.shot img {
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 14px 44px rgba(0,0,0,0.55);
  }
---

<!-- _class: lead -->

![bg fit brightness:0.28](images/01-hero.png)

# Korlap

**A feature tour**

<br>

Run many Claude Code agents in parallel —
each isolated, each watched, all from one board.

---

## A quick note — this is a fork

What you're seeing is **my fork** of Korlap, where I experiment with my own workflow.

For the canonical project, full docs, and releases, **refer to the original repository**.

- **Original:** [ariaghora/korlap](https://github.com/ariaghora/korlap)
- **My fork:** [arham09/korlap](https://github.com/arham09/korlap)

---

## Why I use this — the problem it solves

Running several Claude Code agents at once, the plain tooling fought me at every step.

| Before | With Korlap |
|--------|-------------|
| **Parallel work collides.** One checkout, one branch at a time — stash, switch, stash again. Two tasks can't truly run side by side. | **A `git worktree` per task.** Every agent gets its own working tree + branch. Run many at once; none ever steps on another. |
| **A terminal jungle.** One `claude` per tmux pane, N tabs open, no idea which is mid-task and which is waiting on me. | **One board, every agent.** Switch instantly; status dots show who's running vs. waiting — no terminal-tab roulette. |
| **`git diff` is hard to read.** Raw, unscoped, no highlighting — tough to see what an agent actually changed. | **A real diff view.** Per-workspace, against the merge-base, syntax-highlighted, live — click a hunk to quote it into chat. |

<p class="how"><strong>The point:</strong> isolation + orchestration + visibility — the three things a wall of terminals can't give you.</p>

---

## What's in the box

<div class="cols">
<div>

**Run agents**
- Parallel isolated workspaces
- Kanban-driven lifecycle
- Structured chat
- Diff viewer
- Terminal · Files · Editor

</div>
<div>

**Automate & integrate**
- LSP for agents
- Branch sync 
- `gh` profiles per repo
- Isolated workspaces

</div>
</div>

---

## Parallel isolated workspaces

**Every agent gets its own full repo copy** — a `git worktree` on its own branch.

- Agents never collide: separate working tree, branch, diff, and chat history.
- Each runs an isolated `claude` subprocess with its own session.
- Confined to their worktree — they can't touch other workspaces or the main repo.
- Switching between them is instant; each stays alive in the background.

<p class="how"><strong>How:</strong> create a task → Korlap spins up the worktree, branch, and agent automatically.</p>

---

<!-- _class: shot -->

## The sidebar — many agents, each its own workspace

![h:560](images/03-sidebar-workspaces.png)

---

## A kanban that drives the agents

The board **is** the lifecycle. Dragging a card advances the agent through:

- **Todo** → defined, no agent yet.
- **Plan** → the agent designs the change (plan mode, optional `plan`).
- **In Progress** → full-permission implementation.
- **Review** → a PR is open; keep iterating.
- **Done** → merged; worktree kept for audit.

<p class="how"><strong>Status dots:</strong> pulsing amber = agent running · olive = waiting on you.</p>

---

<!-- _class: shot -->

## The board — cards flow Todo → Done

![h:560](images/02-kanban.png)

---

## Structured chat

Agent output is **parsed, not dumped** — a rich message list, not a raw log.

- From `stream-json`: tool calls, thinking, and results stay distinct.
- Queue messages while the agent is busy.
- Paste **images**, mention **`@file`** paths, run **slash commands**.
- Per-message token counts; full history persisted per workspace.

<p class="how"><strong>Where:</strong> the Chat tab in any workspace.</p>

---

<!-- _class: shot -->

## Structured chat — tool calls, results, tokens

![h:560](images/04-chat.png)

---

## Diff viewer

See exactly what each agent changed — **against the merge-base**, syntax-highlighted.

- Unified diff scoped to the workspace's branch.
- Click a hunk header to **quote it back into chat** as context.
- Updates live as the agent edits.

<p class="how"><strong>Where:</strong> the Diff tab. <span class="tag">⌘R</span> runs an AI Review of the same diff.</p>

---

<!-- _class: shot -->

## Diff against the merge-base, syntax-highlighted

![h:560](images/05-diff.png)

---

## Terminal, Files & Editor

Everything you'd reach for, inside the workspace — no context-switch to another app.

<div class="cols">
<div>

### Terminal
Raw `zsh` PTY tabs, one or many, rooted in the worktree directory. Full native terminal via xterm.js.

</div>
<div>

### Files & Editor
Browse the worktree as a tree; open and edit any file in a CodeMirror 6 editor with multi-language highlighting.

</div>
</div>

<p class="how"><strong>Tip:</strong> the agent owns the worktree — edit by hand sparingly.</p>

---

## LSP — agents that navigate code

A per-repo **language-server pool**, exposed to the agent over MCP.

- Built-in defaults for Rust, TypeScript, Svelte, Go, and more — plus custom servers.
- The agent can **hover, go-to-definition, find-references, rename, and read diagnostics** across the whole worktree.
- Far cheaper and more precise than grepping for symbols.

<p class="how"><strong>Where:</strong> configure & monitor servers in <span class="tag">⌘,</span> → LSP.</p>

---

## Branch sync

Keep long-running branches current without leaving the app.

- The header shows **how many commits you're behind** the base branch.
- One click merges the base in (`git merge origin/<base>` inside the worktree).

<p class="how"><strong>Trigger:</strong> <span class="tag">⌘U</span> — Update branch from base.</p>

---

## Tailor each task & each repo

<div class="cols">
<div>

### Per task
- Plan-mode & Thinking toggles
- **Model override** (Sonnet / Opus / Haiku)
- `@file` mentions & pasted images
- "Add and Start" to spawn instantly

</div>
<div>

### Per repo <span class="tag">⌘,</span>
- `system_prompt`, default model
- `default_start_phase`, `default_plan`
- `openspec_enabled`,

</div>
</div>

---

<!-- _class: lead -->

# You orchestrate. Agents execute.
