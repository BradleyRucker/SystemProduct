<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { initEventListeners, setReadOnly, getReadOnlySetting } from '$lib/store/model';
  import { applyTheme, getPreferredTheme } from '$lib/theme';
  import { authEnabled } from '$lib/auth/nhost';
  import AuthGate from '$lib/auth/AuthGate.svelte';
  import { authState, disposeAuth, initAuth } from '$lib/auth/store';
  import { getCurrentWindow, PhysicalSize, currentMonitor, primaryMonitor } from '@tauri-apps/api/window';

  async function clampWindowToScreen() {
    try {
      const win = getCurrentWindow();
      const monitor = (await currentMonitor()) ?? (await primaryMonitor());
      const max = monitor?.workArea?.size ?? monitor?.size;
      if (!max) return;

      const maxSize = new PhysicalSize(max.width, max.height);
      await win.setMaxSize(maxSize);

      const current = await win.outerSize();
      if (current.width > max.width || current.height > max.height) {
        await win.setSize(
          new PhysicalSize(
            Math.min(current.width, max.width),
            Math.min(current.height, max.height)
          )
        );
      }
    } catch {
      // Ignore if not running in Tauri or API unavailable.
    }
  }

  onMount(() => {
    initEventListeners();
    applyTheme(getPreferredTheme());
    setReadOnly(getReadOnlySetting());
    clampWindowToScreen();
    void initAuth();

    return () => {
      disposeAuth();
    };
  });
</script>

{#if !authEnabled}
  <slot />
{:else if !$authState.initialized || $authState.loading}
  <main class="auth-loading-shell">
    <div class="auth-loading">
      <div class="auth-spinner" aria-hidden="true"></div>
      <span>Checking your session…</span>
    </div>
  </main>
{:else if !$authState.session}
  <AuthGate />
{:else}
  <slot />
{/if}

<style>
  .auth-loading-shell {
    min-height: 100vh;
    display: grid;
    place-items: center;
    background: var(--surface-base);
    color: var(--text-secondary);
    animation: fadeIn var(--transition-normal) ease;
  }

  .auth-loading {
    display: flex;
    align-items: center;
    gap: 10px;
    border: 1px solid var(--surface-border);
    background: var(--surface-overlay);
    padding: 10px 16px;
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    color: var(--text-secondary);
    box-shadow: var(--shadow-md);
  }

  .auth-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--surface-border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.75s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
</style>
