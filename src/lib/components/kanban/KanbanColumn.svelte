<script lang="ts">
  import type { Snippet } from "svelte";
  import { dropTarget, dragStore } from "./dnd.svelte";

  interface Props {
    title: string;
    count: number;
    accent?: boolean;
    dimmed?: boolean;
    /** Column index: 0=Todo, 1=Design, 2=InProgress, 3=Review, 4=Done. */
    col: number;
    /** Returns true when the current drag is allowed to land in this column. */
    accepts?: (drag: NonNullable<typeof dragStore.current>) => boolean;
    children: Snippet;
    footer?: Snippet;
    headerAction?: Snippet;
  }

  let { title, count, accent = false, dimmed = false, col, accepts, children, footer, headerAction }: Props = $props();

  const drag = $derived(dragStore.current);
  const isOver = $derived(drag?.overCol === col);
  const valid = $derived(drag && accepts ? accepts(drag) : true);
</script>

<div
  class="column"
  class:dimmed
  class:drop-hover={isOver}
  class:drop-valid={isOver && valid}
  class:drop-invalid={isOver && !valid}
  use:dropTarget={{ col }}
>
  <div class="column-header">
    <span class="column-title">{title}</span>
    <span class="column-count" class:accent>{count}</span>
    {#if headerAction}
      <span class="header-action-spacer"></span>
      {@render headerAction()}
    {/if}
  </div>
  <div class="column-body">
    {@render children()}
  </div>
  {#if footer}
    <div class="column-footer">
      {@render footer()}
    </div>
  {/if}
</div>

<style>
  .column {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg-sidebar);
    border-radius: 8px;
    border: 1px solid var(--border);
    overflow: hidden;
    transition: border-color 0.12s, background 0.12s;
  }

  .column.dimmed {
    opacity: 0.38;
  }

  .column.dimmed:hover {
    opacity: 0.55;
  }

  .column.drop-valid {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 6%, var(--bg-sidebar));
  }

  .column.drop-invalid {
    border-color: var(--diff-del);
    background: color-mix(in srgb, var(--diff-del) 5%, var(--bg-sidebar));
  }

  .column.drop-invalid * {
    cursor: not-allowed !important;
  }

  .column-header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.6rem 0.75rem;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .column-title {
    font-size: 0.68rem;
    font-weight: 600;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .column-count {
    font-size: 0.6rem;
    font-weight: 600;
    min-width: 1.2rem;
    height: 1.2rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    background: var(--border);
    color: var(--text-dim);
  }

  .column-count.accent {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    color: var(--accent);
  }

  .header-action-spacer {
    flex: 1;
  }

  .column-body {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .column-footer {
    border-top: 1px solid var(--border);
    padding: 0.5rem;
    flex-shrink: 0;
  }
</style>
