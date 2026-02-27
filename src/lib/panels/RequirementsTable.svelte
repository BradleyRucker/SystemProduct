<script lang="ts">
  /**
   * RequirementsTable — spreadsheet-like view of all requirements.
   * Inline editing. No AI in this component — keep it pure data.
   */
  import { nodes } from '$lib/store/model';
  import { saveNode } from '$lib/store/model';
  import type { Node, RequirementData } from '$lib/types';
  import { createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ select: Node }>();

  $: requirements = $nodes.filter((n) => n.kind === 'requirement') as Node[];

  function reqData(node: Node): RequirementData {
    return node.data as RequirementData;
  }

  async function updateField(node: Node, field: keyof RequirementData, value: unknown) {
    const updated: Node = {
      ...node,
      modified_at: new Date().toISOString(),
      data: { ...node.data, [field]: value } as RequirementData,
    };
    await saveNode(updated);
  }

  const PRIORITY_OPTIONS = ['shall', 'should', 'may'] as const;
  const STATUS_OPTIONS   = ['draft', 'approved', 'obsolete'] as const;
  const VERIF_OPTIONS    = ['analysis', 'test', 'inspection', 'demonstration'] as const;
</script>

<div class="req-table-wrapper">
  <table class="req-table">
    <thead>
      <tr>
        <th class="col-id">ID</th>
        <th class="col-name">Name</th>
        <th class="col-text">Requirement Text</th>
        <th class="col-priority">Priority</th>
        <th class="col-status">Status</th>
        <th class="col-verif">Verification</th>
      </tr>
    </thead>
    <tbody>
      {#each requirements as node (node.id)}
        {@const data = reqData(node)}
        <tr on:click={() => dispatch('select', node)}>
          <td class="col-id">
            <span class="req-id">{data.req_id ?? '—'}</span>
          </td>
          <td class="col-name">
            <input
              class="cell-input"
              value={node.name}
              on:change={(e) => saveNode({ ...node, name: e.currentTarget.value, modified_at: new Date().toISOString() })}
              on:click|stopPropagation
            />
          </td>
          <td class="col-text">
            <textarea
              class="cell-textarea"
              value={data.text ?? ''}
              rows="2"
              on:change={(e) => updateField(node, 'text', e.currentTarget.value)}
              on:click|stopPropagation
            ></textarea>
          </td>
          <td class="col-priority">
            <select
              class="cell-select priority-{data.priority}"
              value={data.priority}
              on:change={(e) => updateField(node, 'priority', e.currentTarget.value)}
              on:click|stopPropagation
            >
              {#each PRIORITY_OPTIONS as opt}
                <option value={opt}>{opt}</option>
              {/each}
            </select>
          </td>
          <td class="col-status">
            <select
              class="cell-select"
              value={data.status}
              on:change={(e) => updateField(node, 'status', e.currentTarget.value)}
              on:click|stopPropagation
            >
              {#each STATUS_OPTIONS as opt}
                <option value={opt}>{opt}</option>
              {/each}
            </select>
          </td>
          <td class="col-verif">
            <select
              class="cell-select"
              value={data.verification_method ?? ''}
              on:change={(e) => updateField(node, 'verification_method', e.currentTarget.value || undefined)}
              on:click|stopPropagation
            >
              <option value="">—</option>
              {#each VERIF_OPTIONS as opt}
                <option value={opt}>{opt}</option>
              {/each}
            </select>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>

  {#if requirements.length === 0}
    <div class="empty">No requirements yet.</div>
  {/if}
</div>

<style>
  .req-table-wrapper {
    overflow: auto;
    height: 100%;
  }

  .req-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-sm);
  }

  thead {
    position: sticky;
    top: 0;
    z-index: var(--z-ui);
    background: var(--surface-raised);
  }

  th {
    padding: var(--space-2) var(--space-3);
    text-align: left;
    font-weight: var(--weight-semibold);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    border-bottom: 1px solid var(--surface-border);
    white-space: nowrap;
  }

  tr {
    border-bottom: 1px solid var(--surface-border);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  tr:hover { background: var(--surface-hover); }

  td {
    padding: var(--space-1) var(--space-2);
    vertical-align: top;
  }

  .col-id       { width: 80px; }
  .col-name     { width: 160px; }
  .col-text     { min-width: 280px; }
  .col-priority { width: 90px; }
  .col-status   { width: 90px; }
  .col-verif    { width: 120px; }

  .req-id {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--color-requirement);
    background: var(--color-requirement-bg);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
  }

  .cell-input,
  .cell-textarea,
  .cell-select {
    width: 100%;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: var(--text-sm);
    font-family: var(--font-sans);
    padding: 2px 4px;
    resize: none;
    transition: border-color var(--transition-fast);
  }

  .cell-input:focus,
  .cell-textarea:focus,
  .cell-select:focus {
    outline: none;
    border-color: var(--accent);
    background: var(--surface-overlay);
  }

  .cell-select {
    cursor: pointer;
  }

  .priority-shall  { color: var(--color-error); }
  .priority-should { color: var(--color-warning); }
  .priority-may    { color: var(--text-muted); }

  .empty {
    padding: var(--space-8);
    text-align: center;
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
</style>
