# Korlap — Team Presentation

`korlap.md` is a [Marp](https://marp.app/) markdown deck: a **feature tour** of Korlap for
the team. It opens with a feature map, then spends one slide per capability — parallel
isolated workspaces, the kanban-driven lifecycle, structured chat, diff viewer, terminal/
files/editor, script runner, AI Review, LSP, MCP, knowledge base, Autopilot, branch sync,
`gh` profiles, combo workspaces, and imports — closing with customization and getting started.

It's plain markdown — readable as-is in any editor — but renders as slides with Marp.

## Present / export it

**VS Code (easiest):** install the *Marp for VS Code* extension, open `korlap.md`,
and click the preview icon. Use the "Marp: Export slide deck…" command for PDF/HTML/PPTX.

**CLI** (no install — via `bunx`/`npx`):

```bash
# Live preview server with hot reload
bunx @marp-team/marp-cli -s presentation/

# Export to a self-contained HTML file
bunx @marp-team/marp-cli presentation/korlap.md -o presentation/korlap.html

# Export to PDF (needs a local Chrome/Chromium)
bunx @marp-team/marp-cli presentation/korlap.md --pdf -o presentation/korlap.pdf

# Export to PowerPoint
bunx @marp-team/marp-cli presentation/korlap.md --pptx -o presentation/korlap.pptx
```

## Editing

- Slides are separated by `---` on its own line. The block at the very top is Marp
  front-matter (config + the custom warm-dark amber theme), not a slide.
- Theme styling lives inline under `style:` in the front-matter — tweak the CSS variables
  (`--accent`, `--bg-base`, …) to restyle the whole deck.
- Keep slides terse — they're talking points, not a document. The long-form reference is
  `../docs/USAGE.md`.

## Source material

The deck is distilled from `../README.md`, `../design.md`, and `../docs/USAGE.md`.
If those change substantially, update the deck to match.
