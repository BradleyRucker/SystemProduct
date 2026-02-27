<script context="module" lang="ts">
  export type MenuItem = {
    label: string;
    action: string;
    danger?: boolean;
    disabled?: boolean;
    separator?: boolean;
  };
</script>

<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  type MenuItem = {
    label: string;
    action: string;
    danger?: boolean;
    disabled?: boolean;
    separator?: boolean;
  };

  export let x: number;
  export let y: number;
  export let items: MenuItem[];

  const dispatch = createEventDispatcher<{ select: string; close: void }>();

  // Decide whether to anchor from top or bottom, and left or right,
  // based on where in the viewport the click happened.
  // This is pure math — no DOM measurement needed.
  const MENU_HEIGHT_ESTIMATE = items.length * 36 + 16; // rough: 36px/item + padding
  const MENU_WIDTH = 220;
  const MARGIN = 8;

  const anchorBottom = y + MENU_HEIGHT_ESTIMATE > window.innerHeight - MARGIN;
  const anchorRight  = x + MENU_WIDTH > window.innerWidth - MARGIN;

  const posStyle = [
    anchorBottom ? `bottom: ${window.innerHeight - y}px` : `top: ${y}px`,
    anchorRight  ? `right: ${window.innerWidth - x}px`   : `left: ${x}px`,
  ].join('; ');

  function select(item: MenuItem) {
    if (item.disabled || item.separator) return;
    dispatch('select', item.action);
    dispatch('close');
  }

  function close() {
    dispatch('close');
  }

  function onBackdropContextMenu(e: MouseEvent) {
    e.preventDefault();
    close();
  }
</script>

<svelte:window on:keydown={(e) => e.key === 'Escape' && close()} />

<!-- Backdrop catches outside clicks -->
<div
  class="backdrop"
  on:click={close}
  on:contextmenu={onBackdropContextMenu}
  role="presentation"
></div>

<!-- Menu -->
<div
  class="context-menu"
  style={posStyle}
  role="menu"
>
  {#each items as item}
    {#if item.separator}
      <div class="separator" role="separator"></div>
    {:else}
      <button
        class="menu-item"
        class:danger={item.danger}
        class:disabled={item.disabled}
        role="menuitem"
        disabled={item.disabled}
        on:click={() => select(item)}
      >
        {item.label}
      </button>
    {/if}
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: calc(var(--z-dropdown) - 1);
  }

  .context-menu {
    position: fixed;
    z-index: var(--z-dropdown);
    min-width: 210px;
    max-height: calc(100vh - 16px);
    overflow-y: auto;
    background: linear-gradient(180deg, #162034 0%, var(--surface-float) 100%);
    border: 1px solid var(--surface-border-bright);
    border-radius: var(--radius-lg);
    padding: var(--space-1);
    box-shadow: var(--shadow-float);
    backdrop-filter: blur(14px);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .menu-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: var(--space-2) var(--space-3);
    background: none;
    border: none;
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    font-family: var(--font-sans);
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .menu-item:hover:not(.disabled) {
    background: var(--surface-hover);
    transform: translateX(1px);
  }

  .menu-item.danger {
    color: var(--color-error);
  }

  .menu-item.danger:hover:not(.disabled) {
    background: #ef444420;
  }

  .menu-item.disabled {
    color: var(--text-muted);
    cursor: not-allowed;
  }

  .separator {
    height: 1px;
    background: var(--surface-border);
    margin: var(--space-1) 0;
  }
</style>
