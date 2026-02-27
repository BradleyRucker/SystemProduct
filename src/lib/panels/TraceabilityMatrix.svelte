<script lang="ts">
  /**
   * TraceabilityMatrix — requirements × blocks/test cases cross-reference.
   * Shows which blocks satisfy which requirements and which test cases verify them.
   * Read-only view; click a cell to navigate to that relationship.
   */
  import { nodes, edges } from '$lib/store/model';
  import type { Node, Edge } from '$lib/types';
  import { createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ selectEdge: Edge }>();

  $: requirements = $nodes.filter((n) => n.kind === 'requirement');
  $: blocks       = $nodes.filter((n) => n.kind === 'block');
  $: testCases    = $nodes.filter((n) => n.kind === 'test_case');

  /** Map reqId → Set of block IDs that satisfy it */
  $: satisfiesMap = buildMap($edges, 'satisfies');
  /** Map reqId → Set of test case IDs that verify it */
  $: verifiesMap  = buildMap($edges, 'verifies');

  function buildMap(allEdges: Edge[], kind: string): Map<string, Set<string>> {
    const map = new Map<string, Set<string>>();
    for (const edge of allEdges) {
      if (edge.kind !== kind) continue;
      const set = map.get(edge.target_id) ?? new Set();
      set.add(edge.source_id);
      map.set(edge.target_id, set);
    }
    return map;
  }

  function findEdge(sourceId: string, targetId: string, kind: string): Edge | undefined {
    return $edges.find(
      (e) => e.source_id === sourceId && e.target_id === targetId && e.kind === kind
    );
  }

  function reqLabel(n: Node): string {
    const data = n.data as any;
    return data.req_id ? `${data.req_id}` : n.name;
  }

  type ActiveMode = 'satisfies' | 'verifies';
  let mode: ActiveMode = 'satisfies';
  $: columns = mode === 'satisfies' ? blocks : testCases;
  $: activeMap = mode === 'satisfies' ? satisfiesMap : verifiesMap;
</script>

<div class="matrix-wrapper">
  <div class="matrix-toolbar">
    <button
      class="tab"
      class:active={mode === 'satisfies'}
      on:click={() => (mode = 'satisfies')}
    >Blocks × Requirements</button>
    <button
      class="tab"
      class:active={mode === 'verifies'}
      on:click={() => (mode = 'verifies')}
    >Test Cases × Requirements</button>
  </div>

  {#if requirements.length === 0 || columns.length === 0}
    <div class="empty">
      Add {requirements.length === 0 ? 'requirements' : mode === 'satisfies' ? 'blocks' : 'test cases'} to see the matrix.
    </div>
  {:else}
    <div class="matrix-scroll">
      <table class="matrix-table">
        <thead>
          <tr>
            <th class="corner"></th>
            {#each columns as col (col.id)}
              <th class="col-header">
                <div class="col-label" title={col.name}>{col.name}</div>
              </th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each requirements as req (req.id)}
            <tr>
              <td class="row-header">
                <span class="req-chip">{reqLabel(req)}</span>
                <span class="req-name">{req.name}</span>
              </td>
              {#each columns as col (col.id)}
                {@const linked = activeMap.get(req.id)?.has(col.id) ?? false}
                {@const edge = linked ? findEdge(col.id, req.id, mode) : undefined}
                <td
                  class="cell"
                  class:linked
                  on:click={() => edge && dispatch('selectEdge', edge)}
                  title={linked ? `${col.name} ${mode} ${reqLabel(req)}` : ''}
                >
                  {#if linked}
                    <span class="cell-mark">●</span>
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .matrix-wrapper {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .matrix-toolbar {
    display: flex;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
    border-bottom: 1px solid var(--surface-border);
    flex-shrink: 0;
  }

  .tab {
    padding: var(--space-1) var(--space-3);
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    cursor: pointer;
    font-size: var(--text-sm);
    color: var(--text-secondary);
    transition: all var(--transition-fast);
  }

  .tab:hover { background: var(--surface-hover); color: var(--text-primary); }
  .tab.active {
    background: var(--accent-dim);
    border-color: var(--accent);
    color: var(--accent-hover);
  }

  .matrix-scroll {
    overflow: auto;
    flex: 1;
  }

  .matrix-table {
    border-collapse: collapse;
    font-size: var(--text-sm);
  }

  .corner { min-width: 200px; }

  .col-header {
    padding: var(--space-2);
    border-bottom: 1px solid var(--surface-border);
    border-right: 1px solid var(--surface-border);
    min-width: 80px;
  }

  .col-label {
    writing-mode: vertical-rl;
    transform: rotate(180deg);
    font-size: var(--text-xs);
    color: var(--text-secondary);
    font-weight: var(--weight-medium);
    max-height: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-header {
    padding: var(--space-1) var(--space-2);
    border-bottom: 1px solid var(--surface-border);
    border-right: 1px solid var(--surface-border);
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .req-chip {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--color-requirement);
    background: var(--color-requirement-bg);
    padding: 0 4px;
    border-radius: var(--radius-sm);
    flex-shrink: 0;
  }

  .req-name {
    color: var(--text-secondary);
    font-size: var(--text-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 140px;
  }

  .cell {
    border-bottom: 1px solid var(--surface-border);
    border-right: 1px solid var(--surface-border);
    text-align: center;
    min-width: 80px;
    cursor: default;
    transition: background var(--transition-fast);
  }

  .cell.linked {
    background: var(--color-requirement-bg);
    cursor: pointer;
  }
  .cell.linked:hover { background: var(--accent-dim); }

  .cell-mark {
    color: var(--color-requirement);
    font-size: 10px;
  }

  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
</style>
