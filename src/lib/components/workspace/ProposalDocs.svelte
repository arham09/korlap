<script lang="ts">
  import { listProposalDocs, readFile, type ProposalDoc } from "$lib/ipc";
  import { renderMarkdown } from "$lib/markdown";
  import { externalLinks, copyCodeBlocks, tooltip } from "$lib/actions";
  import { FileText } from "lucide-svelte";
  import ResizeHandle from "../ResizeHandle.svelte";

  interface Props {
    workspaceId: string;
    refreshTrigger?: number;
  }

  let { workspaceId, refreshTrigger = 0 }: Props = $props();

  let docs = $state<ProposalDoc[]>([]);
  let selectedPath = $state<string | null>(null);
  let content = $state("");
  let loading = $state(false);
  let error = $state("");
  let hasLoaded = false;

  function docsKey(d: ProposalDoc[]): string {
    return d.map((x) => x.path).join("\n");
  }
  let lastDocsKey = "";

  async function loadDocs() {
    if (!hasLoaded) loading = true;
    error = "";
    try {
      const next = await listProposalDocs(workspaceId);
      const key = docsKey(next);

      if (next.length === 0) {
        docs = [];
        selectedPath = null;
        content = "";
        lastDocsKey = "";
      } else if (selectedPath && next.some((d) => d.path === selectedPath)) {
        if (key !== lastDocsKey) {
          docs = next;
          lastDocsKey = key;
        }
        // Reload current doc's content (it may have changed on disk)
        await selectDoc(selectedPath, true);
      } else {
        docs = next;
        lastDocsKey = key;
        await selectDoc(next[0].path);
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
      hasLoaded = true;
    }
  }

  async function selectDoc(path: string, keepError = false) {
    selectedPath = path;
    try {
      content = await readFile(workspaceId, path);
      if (!keepError) error = "";
    } catch (e) {
      content = "";
      error = String(e);
    }
  }

  let prevWorkspaceId = "";
  $effect(() => {
    const wsId = workspaceId;
    const _trigger = refreshTrigger;
    if (wsId !== prevWorkspaceId) {
      hasLoaded = false;
      lastDocsKey = "";
      prevWorkspaceId = wsId;
    }
    loadDocs();
  });

  function isMarkdown(path: string): boolean {
    const lower = path.toLowerCase();
    return lower.endsWith(".md") || lower.endsWith(".markdown");
  }

  function fileName(path: string): string {
    return path.split("/").pop() ?? path;
  }

  function fileDir(path: string): string {
    const parts = path.split("/");
    if (parts.length <= 1) return "";
    return parts.slice(0, -1).join("/") + "/";
  }

  let sidebarWidth = $state(240);
  const SIDEBAR_MIN = 140;
  const SIDEBAR_MAX = 500;

  function handleSidebarResize(delta: number) {
    sidebarWidth = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, sidebarWidth + delta));
  }
</script>

<div class="proposal-docs">
  {#if loading}
    <div class="docs-empty">Loading…</div>
  {:else if error && docs.length === 0}
    <div class="docs-empty docs-error">{error}</div>
  {:else if docs.length === 0}
    <div class="docs-empty">
      <p>No proposal docs found.</p>
      <p class="docs-hint">
        Ignored files matching this repo's <strong>Proposal docs</strong> glob
        (set in repo settings) appear here.
      </p>
      <button class="refresh-btn" onclick={loadDocs}>Refresh</button>
    </div>
  {:else}
    <div class="docs-layout">
      <!-- File sidebar -->
      <div class="docs-sidebar" style="width: {sidebarWidth}px">
        <div class="docs-sidebar-header">
          <span class="docs-count">Docs {docs.length}</span>
          <button class="refresh-btn-sm" onclick={loadDocs} use:tooltip={{ text: "Refresh" }}>↻</button>
        </div>
        <div class="docs-list">
          {#each docs as doc}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="docs-item"
              class:active={doc.path === selectedPath}
              onclick={() => selectDoc(doc.path)}
            >
              <FileText size={12} class="docs-item-icon" />
              <span class="docs-path">
                {#if fileDir(doc.path)}<span class="docs-dir">{fileDir(doc.path)}</span>{/if}{fileName(doc.path)}
              </span>
            </div>
          {/each}
        </div>
      </div>
      <ResizeHandle onResize={handleSidebarResize} />

      <!-- Content -->
      <div class="docs-content">
        {#if selectedPath}
          {#if error}
            <div class="docs-error-inline">{error}</div>
          {:else if isMarkdown(selectedPath)}
            <div class="doc-md" use:externalLinks use:copyCodeBlocks>{@html renderMarkdown(content)}</div>
          {:else}
            <pre class="doc-raw">{content}</pre>
          {/if}
        {:else}
          <div class="docs-empty">Select a document</div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .proposal-docs {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .docs-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.6rem;
    color: var(--text-dim);
    font-size: 0.85rem;
    padding: 1rem;
    text-align: center;
  }

  .docs-hint {
    max-width: 360px;
    font-size: 0.78rem;
    line-height: 1.5;
    color: var(--text-muted);
  }

  .docs-error {
    color: var(--diff-del);
  }

  .docs-layout {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  /* ── Sidebar ──────────────────────────── */

  .docs-sidebar {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }

  .docs-sidebar-header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid var(--border);
    font-size: 0.72rem;
  }

  .docs-count {
    color: var(--text-secondary);
    font-weight: 600;
  }

  .refresh-btn-sm {
    margin-left: auto;
    background: none;
    border: none;
    color: var(--text-dim);
    cursor: pointer;
    font-size: 0.85rem;
    padding: 0 0.2rem;
  }

  .refresh-btn-sm:hover {
    color: var(--text-primary);
  }

  .docs-list {
    flex: 1;
    overflow-y: auto;
  }

  .docs-item {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.3rem 0.6rem;
    background: transparent;
    border: none;
    color: var(--text-primary);
    cursor: pointer;
    font-family: inherit;
    font-size: 0.75rem;
    text-align: left;
  }

  .docs-item:hover {
    background: var(--bg-hover);
  }

  .docs-item.active {
    background: var(--border);
  }

  .docs-item :global(.docs-item-icon) {
    flex-shrink: 0;
    color: var(--text-dim);
  }

  .docs-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 0.73rem;
  }

  .docs-dir {
    color: var(--text-dim);
  }

  .refresh-btn {
    padding: 0.25rem 0.6rem;
    background: var(--border);
    border: 1px solid var(--border-light);
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    font-family: inherit;
    font-size: 0.75rem;
  }

  .refresh-btn:hover {
    color: var(--text-primary);
  }

  /* ── Content ──────────────────────────── */

  .docs-content {
    flex: 1;
    overflow: auto;
    min-width: 0;
  }

  .docs-error-inline {
    padding: 1rem;
    color: var(--diff-del);
    font-size: 0.8rem;
  }

  .doc-raw {
    margin: 0;
    padding: 1rem 1.25rem;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    line-height: 1.6;
    color: var(--text-primary);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .doc-md {
    padding: 1rem 1.25rem;
    font-size: 0.85rem;
    color: var(--text-primary);
    max-width: 820px;
  }

  .doc-md :global(h1),
  .doc-md :global(h2),
  .doc-md :global(h3),
  .doc-md :global(h4) {
    margin: 1rem 0 0.4rem;
    color: var(--text-bright);
    font-weight: 600;
    line-height: 1.3;
  }

  .doc-md :global(h1) { font-size: 1.3rem; }
  .doc-md :global(h2) { font-size: 1.12rem; }
  .doc-md :global(h3) { font-size: 1rem; }
  .doc-md :global(h4) { font-size: 0.9rem; }

  .doc-md :global(> :first-child) {
    margin-top: 0;
  }

  .doc-md :global(p) {
    margin: 0.5rem 0;
    line-height: 1.6;
  }

  .doc-md :global(ul),
  .doc-md :global(ol) {
    margin: 0.5rem 0;
    padding-left: 1.5rem;
  }

  .doc-md :global(li) {
    margin: 0.2rem 0;
    line-height: 1.55;
  }

  .doc-md :global(strong) {
    color: var(--text-bright);
    font-weight: 600;
  }

  .doc-md :global(em) {
    font-style: italic;
  }

  .doc-md :global(a) {
    color: var(--accent);
    text-decoration: none;
  }

  .doc-md :global(a:hover) {
    text-decoration: underline;
  }

  .doc-md :global(blockquote) {
    margin: 0.6rem 0;
    padding: 0.1rem 0.9rem;
    border-left: 3px solid var(--border-light);
    color: var(--text-secondary);
  }

  .doc-md :global(code) {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    background: var(--bg-active);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0.1rem 0.35rem;
    color: var(--text-bright);
  }

  .doc-md :global(pre) {
    position: relative;
    margin: 0.6rem 0;
    padding: 0.7rem 0.85rem;
    background: var(--bg-sidebar);
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow-x: auto;
    line-height: 1.5;
  }

  .doc-md :global(pre code) {
    background: none;
    border: none;
    border-radius: 0;
    padding: 0;
    font-size: 0.78rem;
    color: var(--text-primary);
  }

  .doc-md :global(table) {
    border-collapse: collapse;
    margin: 0.6rem 0;
    font-size: 0.8rem;
  }

  .doc-md :global(th),
  .doc-md :global(td) {
    border: 1px solid var(--border);
    padding: 0.3rem 0.6rem;
    text-align: left;
  }

  .doc-md :global(th) {
    background: var(--bg-sidebar);
    color: var(--text-bright);
    font-weight: 600;
  }

  .doc-md :global(hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 1rem 0;
  }

  .doc-md :global(img) {
    max-width: 100%;
  }
</style>
