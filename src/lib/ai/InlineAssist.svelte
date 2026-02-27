<script lang="ts">
  /**
   * InlineAssist — ghost-text AI completion for text fields.
   * Tab to accept, Escape to dismiss. Never activates automatically.
   * Mount this alongside any <textarea> or text <input>.
   */
  import { invoke } from '@tauri-apps/api/core';
  import { aiAvailable } from '$lib/store/model';
  import { createEventDispatcher } from 'svelte';

  export let value: string = '';
  export let context: string = '';   // surrounding model context for the prompt
  export let field: string = '';     // e.g. "req_text" — shown in the prompt

  const dispatch = createEventDispatcher<{ accept: string }>();

  let ghost: string = '';
  let loading = false;
  let visible = false;
  let inputEl: HTMLElement;

  async function requestSuggestion() {
    if (!$aiAvailable || loading || value.trim().length < 3) return;
    loading = true;
    try {
      ghost = await invoke<string>('ai_inline_suggest', {
        field,
        current: value,
        context,
      });
      visible = !!ghost;
    } catch {
      ghost = '';
      visible = false;
    } finally {
      loading = false;
    }
  }

  function accept() {
    if (!ghost) return;
    dispatch('accept', value + ghost);
    dismiss();
  }

  function dismiss() {
    ghost = '';
    visible = false;
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!visible) return;
    if (e.key === 'Tab') {
      e.preventDefault();
      accept();
    } else if (e.key === 'Escape') {
      dismiss();
    } else {
      // Any other key clears the ghost
      dismiss();
    }
  }
</script>

<div class="inline-assist-wrapper">
  <slot
    {requestSuggestion}
    {onKeyDown}
    {dismiss}
    {loading}
  />

  {#if visible && ghost}
    <div class="ghost-text" aria-hidden="true">
      <span class="existing">{value}</span><span class="suggestion">{ghost}</span>
    </div>
    <div class="hint">
      <kbd>Tab</kbd> to accept · <kbd>Esc</kbd> to dismiss
    </div>
  {/if}

  {#if $aiAvailable && !visible && !loading}
    <button
      class="trigger-btn"
      on:click={requestSuggestion}
      title="Get AI suggestion"
      tabindex="-1"
    >
      ✦
    </button>
  {/if}
</div>

<style>
  .inline-assist-wrapper {
    position: relative;
    width: 100%;
  }

  .ghost-text {
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: none;
    white-space: pre-wrap;
    word-break: break-word;
    padding: inherit;
    font: inherit;
    color: transparent;
  }

  .ghost-text .existing {
    color: transparent;
  }

  .ghost-text .suggestion {
    color: var(--text-muted);
    opacity: 0.7;
  }

  .hint {
    position: absolute;
    bottom: -20px;
    right: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    pointer-events: none;
  }

  kbd {
    font-family: var(--font-mono);
    font-size: 10px;
    background: var(--surface-overlay);
    border: 1px solid var(--surface-border);
    border-radius: var(--radius-sm);
    padding: 0 3px;
  }

  .trigger-btn {
    position: absolute;
    top: 50%;
    right: 6px;
    transform: translateY(-50%);
    background: none;
    border: none;
    cursor: pointer;
    font-size: 12px;
    color: var(--text-muted);
    padding: 2px 4px;
    border-radius: var(--radius-sm);
    transition: color var(--transition-fast);
  }

  .trigger-btn:hover {
    color: var(--color-ai-ghost-border);
  }
</style>
