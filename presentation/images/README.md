# Presentation images

Drop screenshots here using the filenames below, and they'll be wired into
`../korlap.md`. PNG preferred. Use the **dark Amber theme** and a consistent
window size; scrub anything sensitive (repo names, tokens, branches).

## Essential (do these first)

| Filename | Slide | What to capture |
|----------|-------|-----------------|
| `01-hero.png` | Title | Whole app window at its best — Plan mode, populated kanban. |
| `02-kanban.png` | Kanban that drives the agents | Board with cards across Todo → Plan → In Progress → Review → Done; status dots visible. |
| `03-sidebar-workspaces.png` | Parallel isolated workspaces | Sidebar with several workspaces + status dots (amber/olive). |
| `04-chat.png` | Structured chat | Chat tab mid-conversation; a tool call + result visible. |
| `05-diff.png` | Diff viewer | Diff tab with a syntax-highlighted diff. |

## Nice-to-have (full coverage)

| Filename | Slide | What to capture |
|----------|-------|-----------------|
| `06-terminal-files.png` | Terminal, Files & Editor | Terminal tab or the file tree + CodeMirror editor. |
| `07-scripts.png` | Script runner | Scripts tab showing the run buttons. |
| `08-ai-review.png` | AI Review | Review pill at top of chat with findings. |
| `09-autopilot.png` | Autopilot | Autopilot pill with streamed events. |
| `10-settings.png` | Tailor each task & repo | Repo Settings open (LSP / MCP / Knowledge tab). |
| `11-task-popover.png` | Tailor each task & repo | New-task popover: model override + plan/thinking toggles. |

## How they'll be used

Most go on the **right half** of their slide as a Marp background
(`![bg right:52%](images/04-chat.png)`); the hero is **full-bleed**
(`![bg](images/01-hero.png)`) behind the title. Capture landscape window
shots — Marp covers/crops to fit, so leave a little breathing room at the edges.
