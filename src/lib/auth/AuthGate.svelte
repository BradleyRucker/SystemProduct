<script lang="ts">
  import { authState, signInWithGoogle, signInWithPassword, signUpWithPassword } from '$lib/auth/store';

  let mode: 'signin' | 'signup' = 'signin';
  let fullName = '';
  let email = '';
  let password = '';
  let localError: string | null = null;
  let notice: string | null = null;

  async function onSubmit() {
    localError = null;
    notice = null;

    const normalizedEmail = email.trim();
    if (!normalizedEmail || !password) {
      localError = 'Email and password are required.';
      return;
    }
    if (password.length < 8) {
      localError = 'Password must be at least 8 characters.';
      return;
    }

    if (mode === 'signin') {
      await signInWithPassword(normalizedEmail, password);
      return;
    }

    await signUpWithPassword(normalizedEmail, password, fullName.trim());
    if (!$authState.error) {
      notice = 'Check your email to confirm your account before signing in.';
    }
  }

  async function onGoogleSignIn() {
    localError = null;
    notice = null;
    await signInWithGoogle();
  }

  $: authError = $authState.error;
  $: visibleError = localError ?? authError;
</script>

<main class="auth-shell">
  <section class="auth-card">
    <div class="auth-brand">Apex</div>
    <h1>{mode === 'signin' ? 'Sign in to your workspace' : 'Create your workspace account'}</h1>
    <p class="auth-subtitle">Use Google or email/password.</p>

    <div class="mode-toggle" role="tablist" aria-label="Authentication mode">
      <button
        type="button"
        class:active={mode === 'signin'}
        on:click={() => {
          mode = 'signin';
          localError = null;
          notice = null;
        }}
      >
        Sign in
      </button>
      <button
        type="button"
        class:active={mode === 'signup'}
        on:click={() => {
          mode = 'signup';
          localError = null;
          notice = null;
        }}
      >
        Sign up
      </button>
    </div>

    <button
      type="button"
      class="google-btn"
      on:click={() => void onGoogleSignIn()}
      disabled={$authState.loading}
    >
      Continue with Google
    </button>

    <div class="divider"><span>or</span></div>

    <form class="auth-form" on:submit|preventDefault={() => void onSubmit()}>
      {#if mode === 'signup'}
        <label>
          Full name
          <input
            type="text"
            placeholder="Jane Doe"
            bind:value={fullName}
            autocomplete="name"
          />
        </label>
      {/if}

      <label>
        Email
        <input
          type="email"
          placeholder="you@gmail.com"
          bind:value={email}
          autocomplete="email"
          required
        />
      </label>

      <label>
        Password
        <input
          type="password"
          placeholder="Minimum 8 characters"
          bind:value={password}
          autocomplete={mode === 'signin' ? 'current-password' : 'new-password'}
          required
        />
      </label>

      <button type="submit" class="btn-primary" disabled={$authState.loading}>
        {#if $authState.loading}
          Working...
        {:else if mode === 'signin'}
          Sign in
        {:else}
          Create account
        {/if}
      </button>
    </form>

    {#if visibleError}
      <div class="auth-error">{visibleError}</div>
    {/if}
    {#if notice}
      <div class="auth-notice">{notice}</div>
    {/if}
  </section>
</main>

<style>
  .auth-shell {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: var(--space-8);
    background:
      radial-gradient(1200px 380px at 80% -100px, #5b6ef526, transparent 70%),
      radial-gradient(1000px 400px at -10% 100%, #22c55e12, transparent 70%),
      var(--surface-base);
  }

  .auth-card {
    width: min(460px, 100%);
    background: var(--surface-overlay);
    border: 1px solid var(--surface-border);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-lg);
    padding: var(--space-6);
    display: grid;
    gap: var(--space-4);
  }

  .auth-brand {
    font-size: var(--text-sm);
    letter-spacing: var(--tracking-wider);
    text-transform: uppercase;
    color: var(--accent-hover);
    font-weight: var(--weight-semibold);
  }

  h1 {
    font-size: var(--text-xl);
    line-height: var(--leading-tight);
    color: var(--text-primary);
  }

  .auth-subtitle {
    color: var(--text-secondary);
    font-size: var(--text-sm);
    margin-top: -6px;
  }

  .mode-toggle {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    padding: 4px;
    border: 1px solid var(--surface-border);
    background: #0b101a;
    border-radius: var(--radius-lg);
  }

  .mode-toggle button {
    border: none;
    background: transparent;
    color: var(--text-muted);
    border-radius: var(--radius-md);
    padding: 8px 10px;
    cursor: pointer;
    font-weight: var(--weight-medium);
    font-size: var(--text-sm);
  }

  .mode-toggle button.active {
    background: var(--accent-dim);
    color: var(--accent-hover);
  }

  .google-btn {
    border: 1px solid var(--surface-border-bright);
    background: #10182a;
    color: var(--text-primary);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    cursor: pointer;
    font-weight: var(--weight-medium);
  }

  .divider {
    display: grid;
    place-items: center;
    position: relative;
    margin: 2px 0;
    color: var(--text-muted);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: var(--tracking-wide);
  }

  .divider::before {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    border-top: 1px solid var(--surface-border-subtle);
    z-index: 0;
  }

  .divider span {
    position: relative;
    z-index: 1;
    background: var(--surface-overlay);
    padding: 0 8px;
  }

  .auth-form {
    display: grid;
    gap: var(--space-3);
  }

  .auth-form label {
    display: grid;
    gap: 6px;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .auth-form input {
    width: 100%;
    border: 1px solid var(--surface-border);
    background: #0e1523;
    color: var(--text-primary);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    font: inherit;
  }

  .auth-form input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 2px #5b6ef530;
  }

  .auth-error,
  .auth-notice {
    border-radius: var(--radius-md);
    padding: 10px 12px;
    font-size: var(--text-sm);
  }

  .auth-error {
    border: 1px solid #ef444460;
    color: #fca5a5;
    background: #ef44441f;
  }

  .auth-notice {
    border: 1px solid #22c55e55;
    color: #86efac;
    background: #22c55e1a;
  }

  button:disabled {
    opacity: 0.65;
    cursor: not-allowed;
  }
</style>
