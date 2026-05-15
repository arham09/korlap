// Kanban drag-and-drop primitives.
//
// Column indices: 0 = Todo, 1 = Design, 2 = In Progress, 3 = Review, 4 = Done.
// Drop validation lives in KanbanBoard; this module is mechanism only.

export type CardType = "todo" | "workspace";

export interface DragInfo {
  cardId: string;
  type: CardType;
  fromCol: number;
  title: string;
  /** Pointer position in viewport coordinates. */
  x: number;
  y: number;
  /** Pointer offset within the original card at dragstart, used to position ghost. */
  offsetX: number;
  offsetY: number;
  /** Column under cursor right now, -1 if none. */
  overCol: number;
  /** Insertion index within overCol (computed against drop targets' children). */
  overIndex: number;
}

class DragStore {
  current = $state<DragInfo | null>(null);
}

export const dragStore = new DragStore();

export interface DraggableOptions {
  cardId: string;
  type: CardType;
  fromCol: number;
  /** Used to render the floating ghost while dragging. */
  title: string;
  /** Called on pointerdown before drag starts; allows the host to cancel keyboard focus etc. */
  onStart?: () => void;
  /** Called on successful drop. (toCol, toIndex) come from the drag snapshot at pointerup. */
  onDrop?: (toCol: number, toIndex: number, drag: DragInfo) => void;
  /** Called on cancel (Escape) or invalid pointerup. */
  onCancel?: () => void;
  /** Pixels of movement required before drag activates (separates click from drag). */
  threshold?: number;
}

const DEFAULT_THRESHOLD = 5;

export function draggable(node: HTMLElement, options: DraggableOptions) {
  let opts = options;
  let pointerId: number | null = null;
  let startX = 0;
  let startY = 0;
  let dragging = false;
  let ghost: HTMLElement | null = null;

  function onPointerDown(e: PointerEvent) {
    // Ignore non-primary buttons and clicks on interactive children (buttons, inputs).
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest("button, input, textarea, a, select")) return;

    pointerId = e.pointerId;
    startX = e.clientX;
    startY = e.clientY;
    dragging = false;
    node.setPointerCapture(pointerId);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("keydown", onKeyDown);
  }

  function onPointerMove(e: PointerEvent) {
    if (pointerId === null || e.pointerId !== pointerId) return;

    if (!dragging) {
      const dx = e.clientX - startX;
      const dy = e.clientY - startY;
      const threshold = opts.threshold ?? DEFAULT_THRESHOLD;
      if (dx * dx + dy * dy < threshold * threshold) return;
      startDrag(e);
    }

    updateDrag(e);
  }

  function startDrag(e: PointerEvent) {
    dragging = true;
    opts.onStart?.();

    const rect = node.getBoundingClientRect();
    dragStore.current = {
      cardId: opts.cardId,
      type: opts.type,
      fromCol: opts.fromCol,
      title: opts.title,
      x: e.clientX,
      y: e.clientY,
      offsetX: e.clientX - rect.left,
      offsetY: e.clientY - rect.top,
      overCol: -1,
      overIndex: -1,
    };

    ghost = document.createElement("div");
    ghost.className = "kanban-drag-ghost";
    ghost.textContent = opts.title;
    Object.assign(ghost.style, {
      position: "fixed",
      top: "0",
      left: "0",
      pointerEvents: "none",
      zIndex: "9999",
      maxWidth: `${Math.min(rect.width, 280)}px`,
      padding: "0.5rem 0.65rem",
      background: "var(--bg-card)",
      border: "1px solid var(--accent)",
      borderRadius: "6px",
      color: "var(--text-bright)",
      fontSize: "0.78rem",
      fontWeight: "600",
      boxShadow: "0 8px 24px rgba(0, 0, 0, 0.35)",
      whiteSpace: "nowrap",
      overflow: "hidden",
      textOverflow: "ellipsis",
      transform: `translate(${e.clientX - (e.clientX - rect.left)}px, ${e.clientY - (e.clientY - rect.top)}px) rotate(2deg)`,
    });
    document.body.appendChild(ghost);
    document.body.style.cursor = "grabbing";
    document.body.style.userSelect = "none";
  }

  function updateDrag(e: PointerEvent) {
    const cur = dragStore.current;
    if (!cur || !ghost) return;
    cur.x = e.clientX;
    cur.y = e.clientY;
    ghost.style.transform = `translate(${e.clientX - cur.offsetX}px, ${e.clientY - cur.offsetY}px) rotate(2deg)`;

    const hit = hitTestColumn(e.clientX, e.clientY);
    cur.overCol = hit.col;
    cur.overIndex = hit.index;
  }

  function onPointerUp(e: PointerEvent) {
    if (pointerId === null || e.pointerId !== pointerId) return;
    const wasDragging = dragging;
    cleanup();

    if (!wasDragging) {
      // It was a click, let the click handler run.
      return;
    }

    const snap = dragStore.current;
    teardownGhost();

    if (snap && snap.overCol >= 0) {
      opts.onDrop?.(snap.overCol, snap.overIndex, snap);
    } else {
      opts.onCancel?.();
    }

    dragStore.current = null;
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!dragging) return;
    if (e.key === "Escape") {
      e.preventDefault();
      cleanup();
      dragStore.current = null;
      teardownGhost();
      opts.onCancel?.();
    }
  }

  function cleanup() {
    if (pointerId !== null) {
      try { node.releasePointerCapture(pointerId); } catch { /* element gone */ }
    }
    pointerId = null;
    dragging = false;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("keydown", onKeyDown);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }

  function teardownGhost() {
    if (ghost) {
      ghost.remove();
      ghost = null;
    }
  }

  node.addEventListener("pointerdown", onPointerDown);

  return {
    update(next: DraggableOptions) {
      opts = next;
    },
    destroy() {
      cleanup();
      teardownGhost();
      node.removeEventListener("pointerdown", onPointerDown);
    },
  };
}

export interface DropTargetOptions {
  col: number;
}

const DROP_COL_ATTR = "data-drop-col";

export function dropTarget(node: HTMLElement, options: DropTargetOptions) {
  node.setAttribute(DROP_COL_ATTR, String(options.col));
  return {
    update(next: DropTargetOptions) {
      node.setAttribute(DROP_COL_ATTR, String(next.col));
    },
    destroy() {
      node.removeAttribute(DROP_COL_ATTR);
    },
  };
}

/**
 * Locate the column under the cursor and the insertion index based on the
 * vertical midpoints of card-like children inside the column body.
 */
function hitTestColumn(x: number, y: number): { col: number; index: number } {
  const els = document.elementsFromPoint(x, y);
  for (const el of els) {
    const colEl = (el as HTMLElement).closest?.(`[${DROP_COL_ATTR}]`) as HTMLElement | null;
    if (!colEl) continue;
    const colStr = colEl.getAttribute(DROP_COL_ATTR);
    const col = colStr ? Number(colStr) : -1;
    if (Number.isNaN(col) || col < 0) continue;
    const body = colEl.querySelector<HTMLElement>(".column-body") ?? colEl;
    const cards = Array.from(body.querySelectorAll<HTMLElement>(":scope > .card"));
    let index = cards.length;
    for (let i = 0; i < cards.length; i++) {
      const r = cards[i].getBoundingClientRect();
      if (y < r.top + r.height / 2) { index = i; break; }
    }
    return { col, index };
  }
  return { col: -1, index: -1 };
}
