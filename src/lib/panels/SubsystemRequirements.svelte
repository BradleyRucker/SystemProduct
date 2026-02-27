<script lang="ts">
  import { nodes } from '$lib/store/model';
  import type { Node, RequirementData } from '$lib/types';

  export let subsystem = '';

  $: requirements = $nodes.filter((n) => n.kind === 'requirement') as Node[];

  function reqData(node: Node): RequirementData {
    return node.data as RequirementData;
  }

  function hasAllocation(node: Node, key: string): boolean {
    const data = node.data as RequirementData;
    const allocations = data.allocations ?? [];
    const target = key.trim().toLowerCase();
    if (!target) return false;
    return allocations.some((entry) => entry.trim().toLowerCase() === target);
  }

  $: allocated = requirements.filter((n) => hasAllocation(n, subsystem));
</script>

<section class="req-panel">
  <div class="panel-header">
    <h2>Allocated Requirements</h2>
    <div class="panel-meta">{allocated.length} item{allocated.length === 1 ? '' : 's'}</div>
  </div>

  {#if allocated.length === 0}
    <div class="empty-state">
      No requirements are allocated to {subsystem} yet.
    </div>
  {:else}
    <div class="req-list">
      {#each allocated as req (req.id)}
        {@const data = reqData(req)}
        <div class="req-card">
          <div class="req-top">
            <div class="req-title">{req.name}</div>
            <div class="req-id">{data.req_id ?? 'REQ-?'} </div>
          </div>
          <div class="req-meta">
            <span class="pill status-{data.status}">{data.status}</span>
            <span class="pill priority-{data.priority}">{data.priority}</span>
            <span class="pill">{data.verification_method ?? 'no verification'}</span>
          </div>
          <div class="req-text">{data.text ?? 'No requirement text yet.'}</div>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .req-panel {
    background: var(--surface-raised);
    border: 1px solid var(--surface-border);
    border-radius: var(--radius-xl);
    padding: var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .panel-header h2 {
    font-size: var(--text-base);
    margin: 0;
  }

  .panel-meta {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .empty-state {
    color: var(--text-secondary);
    font-size: var(--text-sm);
    padding: var(--space-4);
    border: 1px dashed var(--surface-border);
    border-radius: var(--radius-lg);
    text-align: center;
  }

  .req-list {
    display: grid;
    gap: var(--space-3);
  }

  .req-card {
    background: var(--surface-overlay);
    border: 1px solid var(--surface-border);
    border-radius: var(--radius-lg);
    padding: var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .req-top {
    display: flex;
    justify-content: space-between;
    gap: var(--space-2);
    align-items: baseline;
  }

  .req-title {
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
  }

  .req-id {
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-family: var(--font-mono);
  }

  .req-meta {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  .pill {
    font-size: var(--text-xs);
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid var(--surface-border);
    color: var(--text-secondary);
  }

  .status-approved { color: var(--color-success); border-color: #22c55e40; }
  .status-draft { color: var(--text-muted); }
  .status-obsolete { color: var(--color-error); border-color: #ef444440; }

  .priority-shall { color: var(--color-error); border-color: #ef444440; }
  .priority-should { color: var(--color-warning); border-color: #f59e0b40; }
  .priority-may { color: var(--text-secondary); }

  .req-text {
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
</style>
