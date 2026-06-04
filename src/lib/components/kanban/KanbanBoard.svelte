<script lang="ts">
  import type { WorkspaceInfo, PrStatus } from "$lib/ipc";
  import type { PastedImage } from "$lib/chat-utils";
  import KanbanColumn from "./KanbanColumn.svelte";
  import KanbanCard from "./KanbanCard.svelte";
  import CardDetailOverlay from "./CardDetailOverlay.svelte";
  import TaskPopover, { type TaskData } from "./TaskPopover.svelte";
  import ManualCheckoutPopover, { type ManualCheckoutData } from "./ManualCheckoutPopover.svelte";
  import JiraImportPopover, { type JiraTaskData } from "./JiraImportPopover.svelte";
  import PrCheckoutPopover from "./PrCheckoutPopover.svelte";
  import ComboPrCheckoutPopover from "./ComboPrCheckoutPopover.svelte";
  import AutopilotPill, { type AutopilotEvent } from "./AutopilotPill.svelte";
  import { Plus, Ellipsis, Trash2, GitBranch, GitPullRequest, GitMerge, Download } from "lucide-svelte";
  import { tooltip } from "$lib/actions";
  import { addToast } from "$lib/stores/toasts.svelte";
  import type { DragInfo } from "./dnd.svelte";

  interface TodoItem {
    id: string;
    repo_id: string;
    title: string;
    description: string;
    imagePaths?: string[];
    mentionPaths?: string[];
    planMode?: boolean;
    thinkingMode?: boolean;
    model?: string;
    ready?: boolean;
    depends_on?: string[];
    created_at: number;
  }

  interface Props {
    todos: TodoItem[];
    design: WorkspaceInfo[];
    inProgress: WorkspaceInfo[];
    review: WorkspaceInfo[];
    done: WorkspaceInfo[];
    prStatusMap: Map<string, PrStatus>;
    changeCounts: Map<string, { additions: number; deletions: number }>;
    reviewingWsIds: Set<string>;
    creatingWsId: string | null;
    repoId?: string;
    repoName?: string;
    defaultThinkingMode?: boolean;
    defaultPlanMode?: boolean;
    onCardClick: (wsId: string) => void;
    onSpawnAgent: (todoId: string) => void;
    onSpawnDesign: (todoId: string) => void;
    onStartDefault: (todoId: string) => void;
    onAdvanceFromDesign: (wsId: string) => void;
    onNewTodo: (data: TaskData) => void;
    onAddAndStart: (data: TaskData) => void;
    onEditTodo: (todoId: string, data: TaskData) => void;
    onRemoveTodo: (todoId: string) => void;
    onToggleReady: (todoId: string) => void;
    onRemoveWorkspace: (wsId: string) => void;
    onArchiveWorkspace: (wsId: string) => void;
    onRemoveAllDone: () => void;
    onManualCheckout: (data: ManualCheckoutData) => void;
    onPrCheckout: (prNumber: number) => void;
    onComboCheckout: (prNumbers: number[]) => void;
    onReorderTodos?: (orderedIds: string[]) => void;
    onPush?: (wsId: string) => void;
    onMergePr?: (wsId: string) => void;
    autopilotEnabled?: boolean;
    autopilotEvents?: AutopilotEvent[];
    autopilotActiveAgents?: number;
    autopilotMaxAgents?: number;
    autopilotTodoQueue?: number;
    autopilotPrioritizing?: boolean;
    autopilotRebuildingStaging?: boolean;
    onAutopilotCommand?: (command: string) => void;
    active?: boolean;
    /** When false, the Plan column is hidden unless legacy spec-phase workspaces still exist. */
    openspecEnabled?: boolean;
  }

  let {
    todos,
    design,
    inProgress,
    review,
    done,
    prStatusMap,
    changeCounts,
    reviewingWsIds,
    creatingWsId,
    repoId,
    repoName,
    defaultThinkingMode = false,
    defaultPlanMode = false,
    onCardClick,
    onSpawnAgent,
    onSpawnDesign,
    onStartDefault,
    onAdvanceFromDesign,
    onNewTodo,
    onAddAndStart,
    onEditTodo,
    onRemoveTodo,
    onToggleReady,
    onRemoveWorkspace,
    onArchiveWorkspace,
    onRemoveAllDone,
    onManualCheckout,
    onPrCheckout,
    onComboCheckout,
    onReorderTodos,
    onPush,
    onMergePr,
    autopilotEnabled = false,
    autopilotEvents = [],
    autopilotActiveAgents = 0,
    autopilotMaxAgents = 3,
    autopilotTodoQueue = 0,
    autopilotPrioritizing = false,
    autopilotRebuildingStaging = false,
    onAutopilotCommand,
    active = false,
    openspecEnabled = false,
  }: Props = $props();

  // Plan column visible when OpenSpec is on, OR when there are legacy spec
  // workspaces left over from a previous OpenSpec-on state. Drag rules and
  // column rendering both respect this.
  const planColumnVisible = $derived(openspecEnabled || design.length > 0);

  let showAddDialog = $state(false);
  let showManualCheckout = $state(false);
  let showPrCheckout = $state(false);
  let showComboCheckout = $state(false);
  let showJiraImport = $state(false);
  let editingTodo = $state<TodoItem | null>(null);
  let showDoneMenu = $state(false);
  let doneMenuBtnEl = $state<HTMLButtonElement | null>(null);
  let doneMenuPos = $state({ top: 0, left: 0 });
  let showMoreMenu = $state(false);
  let moreMenuBtnEl = $state<HTMLButtonElement | null>(null);
  let moreMenuPos = $state({ top: 0, left: 0 });
  let detailWs = $state<WorkspaceInfo | null>(null);

  export function openNewTask() {
    showAddDialog = true;
  }

  // Keyboard navigation
  let focusedCol = $state(-1); // -1 = no focus
  let focusedRow = $state(0);
  let boardEl = $state<HTMLDivElement | null>(null);

  // Column data as indexable array: [todo, design, inProgress, review, done]
  const columnItems = $derived([todos, design, inProgress, review, done] as const);

  // ── Drag-and-drop rules ────────────────────────────────
  // Allowed forward transitions:
  //   Todo (0) → Todo (0)         reorder
  //   Todo (0) → Design (1)       spawn agent in plan mode
  //   Todo (0) → InProgress (2)   spawn agent (skip design)
  //   Design (1) → InProgress (2) flip to bypassPermissions and start implementing
  //   InProgress (2) → Review (3) push + create PR (modal)
  //   Review (3) → Done (4)       merge PR (modal)
  // Everything else is rejected with a toast.
  const COL_NAMES = ["Todo", "Plan", "In Progress", "Review", "Done"] as const;

  function dragAccepts(drag: DragInfo, toCol: number): boolean {
    if (drag.type === "todo") {
      // Todo can only enter Plan (col 1) when the Plan column is actually visible.
      if (toCol === 1) return planColumnVisible;
      return toCol === 0 || toCol === 2;
    }
    // workspace
    if (drag.fromCol === 1 && toCol === 2) return true;
    if (drag.fromCol === 2 && toCol === 3) return true;
    if (drag.fromCol === 3 && toCol === 4) return true;
    return false;
  }

  function rejectionReason(drag: DragInfo, toCol: number): string {
    if (drag.fromCol === toCol) {
      // same-col reorder: only Todo allowed
      if (toCol === 0) return ""; // shouldn't get here, accepts() returns true
      return `${COL_NAMES[toCol]} is sorted automatically — manual reorder isn't supported.`;
    }
    if (drag.type === "todo" && toCol > 2) {
      return `A todo has no workspace yet — start it first.`;
    }
    if (drag.type === "workspace" && (toCol === 0 || toCol === 1)) {
      if (toCol === 1) return `Workspaces can't go back to Plan — finalize the spec before starting.`;
      return `Use Remove to delete a workspace; it can't go back to Todo.`;
    }
    if (drag.fromCol === 4) {
      return `Done is final — merged PRs can't move back.`;
    }
    if (drag.fromCol === 3 && toCol === 2) {
      return `Review → In Progress would need to close the PR. Use the row menu.`;
    }
    if (drag.fromCol === 2 && toCol === 4) {
      return `Skip Review isn't allowed — drop on Review first.`;
    }
    if (drag.fromCol === 1 && (toCol === 3 || toCol === 4)) {
      return `Plan must go through In Progress first.`;
    }
    return `Move from ${COL_NAMES[drag.fromCol]} to ${COL_NAMES[toCol]} isn't allowed.`;
  }

  function handleDrop(toCol: number, toIndex: number, drag: DragInfo) {
    if (!dragAccepts(drag, toCol)) {
      const reason = rejectionReason(drag, toCol);
      addToast(`Cannot move "${drag.title}" to ${COL_NAMES[toCol]}: ${reason}`, "error");
      return;
    }

    // Todo → Todo reorder
    if (drag.fromCol === 0 && toCol === 0) {
      const fromIdx = todos.findIndex((t) => t.id === drag.cardId);
      if (fromIdx < 0) return;
      const ids = todos.map((t) => t.id);
      ids.splice(fromIdx, 1);
      // toIndex was computed against the live DOM that still contains the dragged card,
      // so account for the removal when it lands after the original slot.
      const insertAt = toIndex > fromIdx ? toIndex - 1 : toIndex;
      ids.splice(Math.max(0, Math.min(insertAt, ids.length)), 0, drag.cardId);
      onReorderTodos?.(ids);
      return;
    }

    // Todo → Design = spawn agent in plan mode
    if (drag.fromCol === 0 && toCol === 1) {
      onSpawnDesign(drag.cardId);
      return;
    }

    // Todo → In Progress = spawn agent (skip design)
    if (drag.fromCol === 0 && toCol === 2) {
      onSpawnAgent(drag.cardId);
      return;
    }

    // Design → In Progress = flip to bypassPermissions and start implementing
    if (drag.fromCol === 1 && toCol === 2) {
      onAdvanceFromDesign(drag.cardId);
      return;
    }

    // In Progress → Review = push to remote (PR creation is a separate manual action)
    if (drag.fromCol === 2 && toCol === 3) {
      if (onPush) onPush(drag.cardId);
      else addToast("Push not wired", "error");
      return;
    }

    // Review → Done = merge PR
    if (drag.fromCol === 3 && toCol === 4) {
      if (onMergePr) onMergePr(drag.cardId);
      else addToast("Merge not wired", "error");
      return;
    }
  }

  function columnAccepts(col: number): (drag: DragInfo) => boolean {
    return (drag) => dragAccepts(drag, col);
  }

  function handleDragStart() {
    focusedCol = -1;
    focusedRow = 0;
  }

  function colLen(col: number): number {
    return columnItems[col]?.length ?? 0;
  }

  // Clamp focus when data changes (e.g. card removed)
  function ensureValidFocus() {
    if (focusedCol < 0) return;
    const len = colLen(focusedCol);
    if (len === 0) {
      focusedCol = -1;
      focusedRow = 0;
    } else {
      focusedRow = Math.min(focusedRow, len - 1);
    }
  }

  function findFirstNonEmptyCol(startDir: "forward" | "backward"): number {
    if (startDir === "forward") {
      for (let i = 0; i < 5; i++) { if (colLen(i) > 0) return i; }
    } else {
      for (let i = 4; i >= 0; i--) { if (colLen(i) > 0) return i; }
    }
    return -1;
  }

  function handleBoardKeydown(e: KeyboardEvent) {
    if (!active) return;
    if (e.defaultPrevented) return;
    if (showAddDialog || showManualCheckout || showPrCheckout || editingTodo || detailWs || showDoneMenu) return;

    const target = e.target as HTMLElement;
    if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) return;

    const key = e.key;
    if (!["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Enter", "Escape"].includes(key)) return;

    e.preventDefault();
    ensureValidFocus();

    switch (key) {
      case "ArrowDown":
      case "ArrowUp": {
        if (focusedCol === -1) {
          const col = findFirstNonEmptyCol("forward");
          if (col >= 0) {
            focusedCol = col;
            focusedRow = key === "ArrowDown" ? 0 : colLen(col) - 1;
          }
        } else {
          if (key === "ArrowDown") {
            focusedRow = Math.min(focusedRow + 1, colLen(focusedCol) - 1);
          } else {
            focusedRow = Math.max(focusedRow - 1, 0);
          }
        }
        break;
      }
      case "ArrowRight":
      case "ArrowLeft": {
        if (focusedCol === -1) {
          const col = findFirstNonEmptyCol(key === "ArrowRight" ? "forward" : "backward");
          if (col >= 0) { focusedCol = col; focusedRow = 0; }
        } else {
          const dir = key === "ArrowRight" ? 1 : -1;
          let next = focusedCol + dir;
          while (next >= 0 && next < 5) {
            if (colLen(next) > 0) {
              focusedCol = next;
              focusedRow = Math.min(focusedRow, colLen(next) - 1);
              break;
            }
            next += dir;
          }
        }
        break;
      }
      case "Enter": {
        if (focusedCol < 0) return;
        if (focusedCol === 0) {
          const todo = todos[focusedRow];
          if (todo) editingTodo = todo;
        } else {
          const lists = [null, design, inProgress, review, done];
          const ws = lists[focusedCol]?.[focusedRow];
          if (ws) {
            if (e.metaKey) onCardClick(ws.id);
            else detailWs = ws;
          }
        }
        break;
      }
      case "Escape": {
        focusedCol = -1;
        focusedRow = 0;
        break;
      }
    }

    // Scroll focused card into view
    if (focusedCol >= 0) {
      requestAnimationFrame(() => {
        boardEl?.querySelector(".card.focused")?.scrollIntoView({ block: "nearest", behavior: "smooth" });
      });
    }
  }

  function openDoneMenu(e: MouseEvent) {
    e.stopPropagation();
    if (doneMenuBtnEl) {
      const rect = doneMenuBtnEl.getBoundingClientRect();
      doneMenuPos = { top: rect.bottom + 4, left: rect.right };
    }
    showDoneMenu = !showDoneMenu;
  }

  function openMoreMenu(e: MouseEvent) {
    e.stopPropagation();
    if (moreMenuBtnEl) {
      const rect = moreMenuBtnEl.getBoundingClientRect();
      moreMenuPos = { top: rect.top - 4, left: rect.left };
    }
    showMoreMenu = !showMoreMenu;
  }

  function handleAddSubmit(data: TaskData) {
    onNewTodo(data);
    showAddDialog = false;
  }

  function handleAddAndStartSubmit(data: TaskData) {
    onAddAndStart(data);
    showAddDialog = false;
  }

  function handleManualCheckoutSubmit(data: ManualCheckoutData) {
    onManualCheckout(data);
    showManualCheckout = false;
  }

  function handlePrCheckoutSelect(prNumber: number) {
    showPrCheckout = false;
    onPrCheckout(prNumber);
  }

  function handleComboSubmit(prNumbers: number[]) {
    showComboCheckout = false;
    onComboCheckout(prNumbers);
  }

  function handleJiraImportSubmit(tasks: JiraTaskData[]) {
    for (const task of tasks) {
      onNewTodo(task);
    }
    showJiraImport = false;
  }

  function handleEditSubmit(data: TaskData) {
    if (editingTodo) {
      onEditTodo(editingTodo.id, data);
      editingTodo = null;
    }
  }
</script>

<svelte:window onkeydown={handleBoardKeydown} />

<div class="kanban-wrapper">
<div class="kanban-board" bind:this={boardEl}>
  <KanbanColumn title="Todo" count={todos.length} col={0} accepts={columnAccepts(0)}>
    {#each todos as todo, i (todo.id)}
      <KanbanCard
        type="todo"
        todoId={todo.id}
        title={todo.title}
        description={todo.description}
        imagePaths={todo.imagePaths}
        planMode={todo.planMode}
        thinkingMode={todo.thinkingMode}
        model={todo.model}
        ready={todo.ready ?? false}
        focused={focusedCol === 0 && focusedRow === i}
        col={0}
        {repoName}
        onAction={() => onStartDefault(todo.id)}
        onEdit={() => { editingTodo = todo; }}
        onRemove={() => onRemoveTodo(todo.id)}
        onToggleReady={() => onToggleReady(todo.id)}
        onDragStart={handleDragStart}
        onDrop={handleDrop}
      />
    {/each}
    {#if todos.length === 0}
      <div class="empty-hint">Add a task to get started</div>
    {/if}
    {#snippet footer()}
      <div class="footer-buttons">
        <button class="add-task-btn" onclick={() => { showAddDialog = true; }} use:tooltip={{ text: "New task", shortcut: "⌘N" }}>
          <Plus size={12} /> New task
        </button>
        <button class="more-btn" bind:this={moreMenuBtnEl} onclick={openMoreMenu} use:tooltip={{ text: "More options" }}>
          <Ellipsis size={14} />
        </button>
      </div>
    {/snippet}
  </KanbanColumn>

  {#if planColumnVisible}
    <KanbanColumn title="Plan" count={design.length} col={1} accepts={columnAccepts(1)}>
      {#each design as ws, i (ws.id)}
        <KanbanCard
          type="workspace"
          workspace={ws}
          prStatus={prStatusMap.get(ws.id)}
          changeCounts={changeCounts.get(ws.id)}
          isReviewing={reviewingWsIds.has(ws.id)}
          isCreating={ws.id === creatingWsId}
          focused={focusedCol === 1 && focusedRow === i}
          col={1}
          onClick={(e) => { e.metaKey ? onCardClick(ws.id) : detailWs = ws; }}
          onRemove={() => onRemoveWorkspace(ws.id)}
          onArchive={() => onArchiveWorkspace(ws.id)}
          onDragStart={handleDragStart}
          onDrop={handleDrop}
        />
      {/each}
      {#if design.length === 0}
        <div class="empty-hint">Drop a task here to plan first</div>
      {/if}
    </KanbanColumn>
  {/if}

  <KanbanColumn title="In Progress" count={inProgress.length} col={2} accepts={columnAccepts(2)}>
    {#each inProgress as ws, i (ws.id)}
      <KanbanCard
        type="workspace"
        workspace={ws}
        prStatus={prStatusMap.get(ws.id)}
        changeCounts={changeCounts.get(ws.id)}
        isReviewing={reviewingWsIds.has(ws.id)}
        isCreating={ws.id === creatingWsId}
        focused={focusedCol === 2 && focusedRow === i}
        col={2}
        onClick={(e) => { e.metaKey ? onCardClick(ws.id) : detailWs = ws; }}
        onRemove={() => onRemoveWorkspace(ws.id)}
        onArchive={() => onArchiveWorkspace(ws.id)}
        onDragStart={handleDragStart}
        onDrop={handleDrop}
      />
    {/each}
    {#if inProgress.length === 0}
      <div class="empty-hint">No agents running</div>
    {/if}
  </KanbanColumn>

  <KanbanColumn title="Review" count={review.length} accent={review.length > 0} col={3} accepts={columnAccepts(3)}>
    {#each review as ws, i (ws.id)}
      <KanbanCard
        type="workspace"
        workspace={ws}
        prStatus={prStatusMap.get(ws.id)}
        changeCounts={changeCounts.get(ws.id)}
        isReviewing={reviewingWsIds.has(ws.id)}
        focused={focusedCol === 3 && focusedRow === i}
        col={3}
        onClick={(e) => { e.metaKey ? onCardClick(ws.id) : detailWs = ws; }}
        onRemove={() => onRemoveWorkspace(ws.id)}
        onArchive={() => onArchiveWorkspace(ws.id)}
        onDragStart={handleDragStart}
        onDrop={handleDrop}
      />
    {/each}
    {#if review.length === 0}
      <div class="empty-hint">Nothing to review</div>
    {/if}
  </KanbanColumn>

  <KanbanColumn title="Done" count={done.length} dimmed col={4} accepts={columnAccepts(4)}>
    {#each done as ws, i (ws.id)}
      <KanbanCard
        type="workspace"
        workspace={ws}
        prStatus={prStatusMap.get(ws.id)}
        changeCounts={changeCounts.get(ws.id)}
        focused={focusedCol === 4 && focusedRow === i}
        col={4}
        onClick={(e) => { e.metaKey ? onCardClick(ws.id) : detailWs = ws; }}
        onRemove={() => onRemoveWorkspace(ws.id)}
        onDragStart={handleDragStart}
        onDrop={handleDrop}
      />
    {/each}
    {#if done.length === 0}
      <div class="empty-hint">Completed tasks appear here</div>
    {/if}
    {#snippet headerAction()}
      {#if done.length > 0}
        <button
          class="column-menu-btn"
          bind:this={doneMenuBtnEl}
          onclick={openDoneMenu}
        >
          <Ellipsis size={14} />
        </button>
      {/if}
    {/snippet}
  </KanbanColumn>
</div>

<AutopilotPill
  enabled={autopilotEnabled}
  events={autopilotEvents}
  activeAgentCount={autopilotActiveAgents}
  maxAgents={autopilotMaxAgents}
  todoQueueLength={autopilotTodoQueue}
  prioritizing={autopilotPrioritizing}
  rebuildingStaging={autopilotRebuildingStaging}
  onSendCommand={(cmd) => onAutopilotCommand?.(cmd)}
  onCardClick={onCardClick}
/>
</div>

{#if showAddDialog}
  <TaskPopover
    {repoId}
    initialThinkingMode={defaultThinkingMode}
    initialPlanMode={defaultPlanMode}
    submitLabel="Add"
    onSubmit={handleAddSubmit}
    onSubmitAndStart={handleAddAndStartSubmit}
    onCancel={() => { showAddDialog = false; }}
  />
{/if}

{#if showManualCheckout}
  <ManualCheckoutPopover
    onSubmit={handleManualCheckoutSubmit}
    onCancel={() => { showManualCheckout = false; }}
  />
{/if}

{#if showPrCheckout && repoId}
  <PrCheckoutPopover
    {repoId}
    onSelect={handlePrCheckoutSelect}
    onCancel={() => { showPrCheckout = false; }}
  />
{/if}

{#if showComboCheckout && repoId}
  <ComboPrCheckoutPopover
    {repoId}
    onSubmit={handleComboSubmit}
    onCancel={() => { showComboCheckout = false; }}
  />
{/if}

{#if showJiraImport}
  <JiraImportPopover
    onSubmit={handleJiraImportSubmit}
    onCancel={() => { showJiraImport = false; }}
  />
{/if}

{#if editingTodo}
  <TaskPopover
    {repoId}
    initialTitle={editingTodo.title}
    initialDescription={editingTodo.description}
    initialImagePaths={editingTodo.imagePaths}
    initialMentions={editingTodo.mentionPaths?.map((p: string) => ({ type: "file" as const, path: p, displayName: p.split("/").pop() ?? p }))}
    initialPlanMode={editingTodo.planMode}
    initialThinkingMode={editingTodo.thinkingMode}
    initialModel={editingTodo.model}
    submitLabel="Save"
    onSubmit={handleEditSubmit}
    onCancel={() => { editingTodo = null; }}
  />
{/if}

{#if detailWs}
  <CardDetailOverlay
    workspace={detailWs}
    prStatus={prStatusMap.get(detailWs.id)}
    changeCounts={changeCounts.get(detailWs.id)}
    isReviewing={reviewingWsIds.has(detailWs.id)}
    isCreating={detailWs.id === creatingWsId}
    onGoToWorkspace={() => { const id = detailWs!.id; detailWs = null; onCardClick(id); }}
    onClose={() => { detailWs = null; }}
  />
{/if}

{#if showDoneMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="dropdown-backdrop" onmousedown={() => { showDoneMenu = false; }}></div>
  <div class="dropdown-menu" style="top: {doneMenuPos.top}px; left: {doneMenuPos.left}px;">
    <button
      class="dropdown-item danger"
      onclick={() => { showDoneMenu = false; onRemoveAllDone(); }}
    >
      <Trash2 size={12} />
      Remove all
    </button>
  </div>
{/if}

{#if showMoreMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="dropdown-backdrop" onmousedown={() => { showMoreMenu = false; }}></div>
  <div class="dropdown-menu dropdown-menu-up" style="top: {moreMenuPos.top}px; left: {moreMenuPos.left}px;">
    <button
      class="dropdown-item"
      onclick={() => { showMoreMenu = false; showManualCheckout = true; }}
    >
      <GitBranch size={12} />
      Manual checkout
    </button>
    <button
      class="dropdown-item"
      onclick={() => { showMoreMenu = false; showPrCheckout = true; }}
    >
      <GitPullRequest size={12} />
      Review PR
    </button>
    <button
      class="dropdown-item"
      onclick={() => { showMoreMenu = false; showComboCheckout = true; }}
    >
      <GitMerge size={12} />
      Combo PRs
    </button>
    <button
      class="dropdown-item"
      onclick={() => { showMoreMenu = false; showJiraImport = true; }}
    >
      <Download size={12} />
      Import from Jira
    </button>
  </div>
{/if}

<style>
  .kanban-wrapper {
    position: relative;
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .kanban-board {
    display: flex;
    gap: 0.75rem;
    padding: 0.75rem;
    flex: 1;
    min-height: 0;
    min-width: 0;
  }

  .empty-hint {
    font-size: 0.72rem;
    color: var(--text-muted);
    text-align: center;
    padding: 1.5rem 0.5rem;
  }

  .footer-buttons {
    display: flex;
    gap: 0.35rem;
  }

  .add-task-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.3rem;
    padding: 0.45rem;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 25%, transparent);
    border-radius: 6px;
    color: var(--accent);
    font-family: inherit;
    font-size: 0.78rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
  }

  .add-task-btn:hover {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .more-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    flex-shrink: 0;
    padding: 0;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-dim);
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
  }

  .more-btn:hover {
    background: var(--border);
    border-color: var(--border-light);
    color: var(--text-secondary);
  }

  .column-menu-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--text-dim);
    cursor: pointer;
    opacity: 0.6;
  }

  .column-menu-btn:hover {
    background: var(--border);
    opacity: 1;
  }

  .dropdown-backdrop {
    position: fixed;
    inset: 0;
    z-index: 99;
  }

  .dropdown-menu {
    position: fixed;
    min-width: 140px;
    background: var(--bg-sidebar);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px;
    z-index: 100;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
  }

  .dropdown-menu-up {
    transform: translateY(-100%);
  }

  .dropdown-item {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
    padding: 0.35rem 0.5rem;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--text-dim);
    font-family: inherit;
    font-size: 0.75rem;
    cursor: pointer;
    text-align: left;
  }

  .dropdown-item:hover {
    background: var(--border);
    color: var(--text);
  }

  .dropdown-item.danger:hover {
    background: color-mix(in srgb, #e05252 15%, transparent);
    color: #e05252;
  }
</style>
