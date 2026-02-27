<script lang="ts">
  /**
   * ModelTree — hierarchical browser of all nodes in the project.
   * Grouped by kind. Click to select.
   *
   * Drag-and-drop uses pointer events (not HTML5 drag API) because Tauri's
   * WebView intercepts OS-level drag events before they reach the DOM.
   */
  import { nodes, validationIssues } from '$lib/store/model';
  import { selectNode } from '$lib/store/canvas';
  import type { Node, NodeKind } from '$lib/types';

  const KIND_ORDER: NodeKind[] = [
    'requirement', 'block', 'interface', 'use_case',
    'actor', 'port', 'test_case', 'function', 'stakeholder',
    'external',
  ];

  const KIND_LABELS: Record<NodeKind, string> = {
    requirement: 'Requirements',
    block:       'Blocks',
    interface:   'Interfaces',
    port:        'Ports',
    use_case:    'Use Cases',
    actor:       'Actors',
    test_case:   'Test Cases',
    function:    'Functions',
    stakeholder: 'Stakeholders',
    external:    'External',
  };

  export let enableDrag = false;
  export let defaultExpanded: NodeKind[] = ['requirement', 'block'];
  /**
   * Called when the user drops a node onto the canvas.
   * clientX/Y are viewport coordinates of the drop point.
   */
  export let onNodeDropped: (nodeId: string, clientX: number, clientY: number) => void = () => {};

  let expandedKinds = new Set<NodeKind>(defaultExpanded);

  $: grouped = groupByKind($nodes);
  $: issueNodeIds = new Set($validationIssues.map((i) => i.node_id).filter(Boolean));

  // ── Pointer-based drag state ───────────────────────────────────────────────
  let draggingNode: Node | null = null;
  let ghostX = 0;
  let ghostY = 0;
  let ghostVisible = false;
  const DRAG_THRESHOLD = 6; // px before drag starts
  let pointerDownPos = { x: 0, y: 0 };
  let pointerDownNode: Node | null = null;

  function groupByKind(allNodes: Node[]): Map<NodeKind, Node[]> {
    const map = new Map<NodeKind, Node[]>();
    for (const kind of KIND_ORDER) {
      const group = allNodes.filter((n) => n.kind === kind);
      if (group.length > 0) map.set(kind, group);
    }
    return map;
  }

  function toggleKind(kind: NodeKind) {
    if (expandedKinds.has(kind)) {
      expandedKinds.delete(kind);
    } else {
      expandedKinds.add(kind);
    }
    expandedKinds = new Set(expandedKinds);
  }

  function onNodeClick(node: Node) {
    if (!draggingNode) selectNode(node.id);
  }

  function onPointerDown(e: PointerEvent, node: Node) {
    if (!enableDrag) return;
    if (e.button !== 0) return;
    pointerDownNode = node;
    pointerDownPos = { x: e.clientX, y: e.clientY };
    // Capture pointer so we get move/up even outside the element
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!enableDrag || !pointerDownNode) return;
    const dx = e.clientX - pointerDownPos.x;
    const dy = e.clientY - pointerDownPos.y;
    if (!draggingNode && Math.sqrt(dx * dx + dy * dy) > DRAG_THRESHOLD) {
      draggingNode = pointerDownNode;
      ghostVisible = true;
    }
    if (draggingNode) {
      ghostX = e.clientX + 12;
      ghostY = e.clientY + 12;
    }
  }

  function onPointerUp(e: PointerEvent) {
    if (!enableDrag) return;
    const node = draggingNode;
    draggingNode = null;
    ghostVisible = false;
    pointerDownNode = null;

    if (!node) return; // was just a click, not a drag

    // Find what element is under the pointer. We use elementFromPoint on the
    // document so we can check if it's inside the canvas area.
    // Release capture first so elementFromPoint works correctly.
    try { (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId); } catch (_) {}

    const el = document.elementFromPoint(e.clientX, e.clientY);
    // Accept drop if pointer is over a <canvas> or any element inside .canvas-area
    const inCanvas = el && (
      el.tagName === 'CANVAS' ||
      el.closest('.canvas-area') !== null
    );
    if (inCanvas) {
      onNodeDropped(node.id, e.clientX, e.clientY);
    }
  }

  function reqId(node: Node): string {
    if (node.kind === 'requirement' && node.data.kind === 'requirement') {
      return node.data.req_id ?? '';
    }
    return '';
  }
</script>

<!-- Ghost label that follows the cursor during drag -->
{#if ghostVisible && draggingNode}
  <div
    class="drag-ghost"
    style="left: {ghostX}px; top: {ghostY}px;"
    aria-hidden="true"
  >
    <span class="ghost-dot kind-{draggingNode.kind}"></span>
    {draggingNode.name || '(unnamed)'}
  </div>
{/if}

<nav class="model-tree" aria-label="Model browser">
  <div class="tree-header">Model</div>

  {#each KIND_ORDER as kind}
    {#if grouped.has(kind)}
      {@const group = grouped.get(kind) ?? []}
      <div class="kind-group">
        <button
          class="kind-header"
          on:click={() => toggleKind(kind)}
          aria-expanded={expandedKinds.has(kind)}
        >
          <span class="chevron" class:open={expandedKinds.has(kind)}>›</span>
          <span class="kind-label">{KIND_LABELS[kind]}</span>
          <span class="count">{group.length}</span>
        </button>

        {#if expandedKinds.has(kind)}
          <ul class="node-list">
            {#each group as node (node.id)}
              <li>
                <button
                  class="node-item"
                  class:has-issue={issueNodeIds.has(node.id)}
                  class:dragging={draggingNode?.id === node.id}
                  on:click={() => onNodeClick(node)}
                  on:pointerdown={(e) => onPointerDown(e, node)}
                  on:pointermove={onPointerMove}
                  on:pointerup={onPointerUp}
                  on:pointercancel={() => { draggingNode = null; ghostVisible = false; pointerDownNode = null; }}
                >
                  <span class="node-dot kind-{kind}"></span>
                  {#if reqId(node)}
                    <span class="req-id-chip">{reqId(node)}</span>
                  {/if}
                  <span class="node-name">{node.name || '(unnamed)'}</span>
                  {#if issueNodeIds.has(node.id)}
                    <span class="issue-dot" title="Has validation issues"></span>
                  {/if}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  {/each}

  {#if $nodes.length === 0}
    <div class="empty-state">No model elements yet.</div>
  {/if}
</nav>

<style>
  .model-tree {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
    padding: var(--space-2) 0;
  }

  .tree-header {
    font-size: var(--text-xs);
    font-weight: var(--weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    padding: var(--space-2) var(--space-4);
    margin-bottom: var(--space-1);
  }

  .kind-header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-1) var(--space-3);
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-secondary);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    text-align: left;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
  }

  .kind-header:hover {
    background: var(--surface-hover);
    color: var(--text-primary);
  }

  .chevron {
    font-size: 12px;
    transition: transform var(--transition-fast);
    display: inline-block;
  }
  .chevron.open { transform: rotate(90deg); }

  .kind-label { flex: 1; }

  .count {
    font-size: var(--text-xs);
    color: var(--text-muted);
    background: var(--surface-overlay);
    padding: 0 5px;
    border-radius: 99px;
  }

  .node-list {
    list-style: none;
    padding: 0;
  }

  .node-item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    padding: 3px var(--space-3) 3px calc(var(--space-3) + 14px);
    background: none;
    border: none;
    cursor: grab;
    color: var(--text-secondary);
    font-size: var(--text-sm);
    text-align: left;
    border-radius: var(--radius-sm);
    transition: background var(--transition-fast);
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
    touch-action: none;
  }

  .node-item:hover {
    background: var(--surface-hover);
    color: var(--text-primary);
  }

  .node-item.has-issue {
    color: var(--color-warning);
  }

  .node-item.dragging {
    opacity: 0.45;
    cursor: grabbing;
  }

  .node-dot {
    flex-shrink: 0;
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .node-dot.kind-requirement { background: var(--color-requirement); }
  .node-dot.kind-block       { background: var(--color-block); }
  .node-dot.kind-interface   { background: var(--color-interface); }
  .node-dot.kind-port        { background: var(--color-port); }
  .node-dot.kind-use_case    { background: var(--color-use-case); }
  .node-dot.kind-actor       { background: var(--color-actor); }
  .node-dot.kind-test_case   { background: var(--color-test-case); }
  .node-dot.kind-function    { background: var(--color-function); }
  .node-dot.kind-stakeholder { background: var(--color-stakeholder); }

  .req-id-chip {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--color-requirement);
    background: var(--color-requirement-bg);
    padding: 0 4px;
    border-radius: var(--radius-sm);
    flex-shrink: 0;
  }

  .node-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .issue-dot {
    flex-shrink: 0;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--color-warning);
  }

  .empty-state {
    padding: var(--space-4);
    color: var(--text-muted);
    font-size: var(--text-sm);
    text-align: center;
  }

  /* Ghost label that floats under the cursor during a drag */
  .drag-ghost {
    position: fixed;
    z-index: 9999;
    pointer-events: none;
    background: var(--surface-raised, #1e2533);
    color: var(--text-primary, #e8ecf4);
    border: 1px solid var(--border-subtle, #2a3347);
    border-radius: var(--radius-sm, 4px);
    padding: 3px 8px;
    font-size: var(--text-sm, 12px);
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
    box-shadow: 0 4px 12px rgba(0,0,0,0.4);
  }

  .ghost-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .ghost-dot.kind-requirement { background: var(--color-requirement); }
  .ghost-dot.kind-block       { background: var(--color-block); }
  .ghost-dot.kind-interface   { background: var(--color-interface); }
  .ghost-dot.kind-port        { background: var(--color-port); }
  .ghost-dot.kind-use_case    { background: var(--color-use-case); }
  .ghost-dot.kind-actor       { background: var(--color-actor); }
  .ghost-dot.kind-test_case   { background: var(--color-test-case); }
  .ghost-dot.kind-function    { background: var(--color-function); }
  .ghost-dot.kind-stakeholder { background: var(--color-stakeholder); }
</style>
