<script lang="ts">
  /**
   * AnalysisBadge — passive analysis indicator in the sidebar.
   * Shows a count badge when the backend has analysis suggestions.
   * Never pops up. Never steals focus. User clicks when they want to see it.
   */
  import type { AiSuggestion } from '$lib/types';

  export let suggestions: AiSuggestion[] = [];
  export let onOpen: () => void = () => {};

  $: errors   = suggestions.filter((s) => s.severity === 'error').length;
  $: warnings = suggestions.filter((s) => s.severity === 'warning').length;
  $: infos    = suggestions.filter((s) => s.severity === 'info').length;
  $: total    = suggestions.length;
</script>

{#if total > 0}
  <button class="badge" on:click={onOpen} title="AI analysis — {total} item{total !== 1 ? 's' : ''}">
    <span class="icon">✦</span>
    {#if errors > 0}
      <span class="count error">{errors}</span>
    {/if}
    {#if warnings > 0}
      <span class="count warning">{warnings}</span>
    {/if}
    {#if infos > 0 && errors === 0 && warnings === 0}
      <span class="count info">{infos}</span>
    {/if}
  </button>
{/if}

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    border-radius: 99px;
    border: 1px solid var(--surface-border);
    background: var(--surface-overlay);
    cursor: pointer;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    transition: background var(--transition-fast);
  }

  .badge:hover {
    background: var(--surface-hover);
    color: var(--text-primary);
  }

  .icon {
    font-size: 10px;
    color: var(--color-ai-ghost-border);
  }

  .count {
    font-weight: var(--weight-semibold);
    font-size: var(--text-xs);
  }

  .count.error   { color: var(--color-error); }
  .count.warning { color: var(--color-warning); }
  .count.info    { color: var(--color-info); }
</style>
