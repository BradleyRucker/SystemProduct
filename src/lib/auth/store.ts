import { writable } from 'svelte/store';
import type { User } from '@nhost/nhost-js/auth';
import type { Session } from '@nhost/nhost-js/session';
import { authEnabled, nhost } from '$lib/auth/nhost';

type AuthState = {
  initialized: boolean;
  loading: boolean;
  session: Session | null;
  user: User | null;
  error: string | null;
};

const initialState: AuthState = {
  initialized: false,
  loading: false,
  session: null,
  user: null,
  error: null,
};

export const authState = writable<AuthState>(initialState);

let authBootstrapped = false;
let unlistenAuthChange: (() => void) | null = null;
let oauthInFlight = false;

function setError(message: string | null) {
  authState.update((s) => ({ ...s, error: message }));
}

function normalizeAuthError(message: string): string {
  const code = message.trim();
  if (code === 'disabled-endpoint') {
    return 'Google sign-in endpoint is disabled in Nhost. Enable Google under Authentication -> Sign-in methods and set Client ID/Secret.';
  }
  if (code === 'redirectTo-not-allowed') {
    return 'OAuth redirect URL is not allowed. Add your app URL in Nhost allowed redirect URLs.';
  }
  if (code === 'invalid-state') {
    return 'OAuth state validation failed. Retry once, and if it persists clear auth cookies/state and ensure OAuth2 Provider (separate feature) is disabled in Nhost.';
  }
  return message;
}

function errorMessage(error: unknown): string {
  if (
    error &&
    typeof error === 'object' &&
    'body' in error &&
    error.body &&
    typeof error.body === 'object'
  ) {
    const body = error.body as { message?: string; error?: string };
    if (typeof body.message === 'string' && body.message.trim()) {
      return normalizeAuthError(body.message);
    }
    if (typeof body.error === 'string' && body.error.trim()) {
      return normalizeAuthError(body.error);
    }
  }
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return normalizeAuthError(error);
  return 'Authentication request failed.';
}

const OAUTH_URL_PARAM_KEYS = [
  'refreshToken',
  'refresh_token',
  'refresh-token',
  'ticket',
  'type',
  'code',
  'state',
  'error',
  'error_description',
  'message',
] as const;

function readLocationParams() {
  const search = new URLSearchParams(window.location.search);
  const hashRaw = window.location.hash.startsWith('#')
    ? window.location.hash.slice(1)
    : window.location.hash;
  const hashLooksLikeParams = hashRaw.includes('=');
  const hash = new URLSearchParams(hashLooksLikeParams ? hashRaw : '');
  const merged = new URLSearchParams();

  for (const [key, value] of search.entries()) merged.append(key, value);
  for (const [key, value] of hash.entries()) merged.append(key, value);

  return { search, hash, hashRaw, hashLooksLikeParams, merged };
}

function clearAuthParamsFromUrl() {
  if (typeof window === 'undefined') return;
  const { search, hash, hashRaw, hashLooksLikeParams } = readLocationParams();
  for (const key of OAUTH_URL_PARAM_KEYS) {
    search.delete(key);
    hash.delete(key);
  }
  const cleanSearch = search.toString();
  const cleanHash = hashLooksLikeParams ? hash.toString() : hashRaw;
  const clean = `${window.location.pathname}${cleanSearch ? `?${cleanSearch}` : ''}${cleanHash ? `#${cleanHash}` : ''}`;
  window.history.replaceState({}, document.title, clean);
}

function extractAuthUrlParams():
  | {
      refreshToken?: string;
      ticket?: string;
      error?: string;
      hasOAuthParams: boolean;
      keys: string[];
    }
  | null {
  if (typeof window === 'undefined') return null;

  const { merged } = readLocationParams();

  const read = (...keys: string[]) =>
    keys
      .map((k) => merged.get(k)?.trim())
      .find((v) => typeof v === 'string' && v.length > 0);

  const refreshToken = read('refreshToken', 'refresh_token', 'refresh-token');
  const ticket = read('ticket');
  const error = read('error_description', 'error', 'message');
  const keys = Array.from(new Set(Array.from(merged.keys())));
  const knownKeys = new Set(OAUTH_URL_PARAM_KEYS);
  const hasOAuthParams = keys.some((key) => knownKeys.has(key as (typeof OAUTH_URL_PARAM_KEYS)[number]));

  if (!refreshToken && !ticket && !error && !hasOAuthParams) return null;
  return { refreshToken, ticket, error, hasOAuthParams, keys };
}

async function consumeOAuthCallbackIfPresent(): Promise<string | null> {
  if (!nhost) return null;
  const callback = extractAuthUrlParams();
  if (!callback) return null;

  if (import.meta.env.DEV) {
    console.info('[auth] OAuth callback keys:', callback.keys.join(', ') || '(none)');
  }

  if (callback.error) {
    clearAuthParamsFromUrl();
    return normalizeAuthError(callback.error);
  }

  if (callback.ticket && typeof window !== 'undefined') {
    const redirectTo = `${window.location.origin}${window.location.pathname}`;
    const verifyUrl = nhost.auth.verifyTicketURL({
      ticket: callback.ticket,
      redirectTo,
    });
    window.location.assign(verifyUrl);
    return null;
  }

  if (!callback.refreshToken) {
    clearAuthParamsFromUrl();
    if (callback.hasOAuthParams) {
      return 'OAuth callback did not return a refresh token. Check Nhost allowed redirects and Google OAuth redirect URI settings.';
    }
    return null;
  }

  try {
    const refreshed = await nhost.auth.refreshToken({ refreshToken: callback.refreshToken });
    if (refreshed.body?.accessToken && refreshed.body.refreshToken) {
      nhost.sessionStorage.set(refreshed.body);
    }
    clearAuthParamsFromUrl();
    return null;
  } catch (error) {
    clearAuthParamsFromUrl();
    return errorMessage(error);
  }
}

export async function initAuth() {
  if (!authEnabled || !nhost) {
    authState.set({
      initialized: true,
      loading: false,
      session: null,
      user: null,
      error: null,
    });
    return;
  }
  if (authBootstrapped) return;
  authBootstrapped = true;

  try {
    authState.update((s) => ({ ...s, loading: true }));
    const callbackError = await consumeOAuthCallbackIfPresent();
    const session = nhost.getUserSession();
    authState.set({
      initialized: true,
      loading: false,
      session,
      user: session?.user ?? null,
      error: callbackError ?? null,
    });
  } catch (error) {
    authState.set({
      initialized: true,
      loading: false,
      session: null,
      user: null,
      error: errorMessage(error),
    });
  }

  unlistenAuthChange = nhost.sessionStorage.onChange((session) => {
    authState.update((s) => ({
      ...s,
      initialized: true,
      loading: false,
      session,
      user: session?.user ?? null,
      error: null,
    }));
  });
}

export async function signInWithPassword(email: string, password: string) {
  if (!nhost) return;
  setError(null);
  authState.update((s) => ({ ...s, loading: true }));
  try {
    await nhost.auth.signInEmailPassword({
      email: email.trim(),
      password,
    });
    const session = nhost.getUserSession();
    authState.update((s) => ({
      ...s,
      loading: false,
      session,
      user: session?.user ?? null,
      error: null,
    }));
  } catch (error) {
    authState.update((s) => ({ ...s, loading: false, error: errorMessage(error) }));
  }
}

export async function signUpWithPassword(
  email: string,
  password: string,
  fullName?: string
) {
  if (!nhost) return;
  setError(null);
  authState.update((s) => ({ ...s, loading: true }));
  const redirectTo = (import.meta.env.VITE_NHOST_REDIRECT_TO as string | undefined)?.trim();
  try {
    await nhost.auth.signUpEmailPassword({
      email: email.trim(),
      password,
      options: {
        displayName: (fullName ?? '').trim() || undefined,
        redirectTo: redirectTo || undefined,
      },
    });
    const session = nhost.getUserSession();
    authState.update((s) => ({
      ...s,
      loading: false,
      session,
      user: session?.user ?? null,
      error: null,
    }));
  } catch (error) {
    authState.update((s) => ({ ...s, loading: false, error: errorMessage(error) }));
  }
}

export async function signInWithGoogle() {
  if (!nhost) return;
  if (oauthInFlight) return;
  oauthInFlight = true;
  setError(null);
  authState.update((s) => ({ ...s, loading: true }));
  try {
    const configuredRedirect = (import.meta.env.VITE_NHOST_REDIRECT_TO as
      | string
      | undefined)
      ?.trim();
    const redirectTo =
      configuredRedirect ||
      (typeof window !== 'undefined' ? `${window.location.origin}/` : undefined);
    const url = nhost.auth.signInProviderURL('google', {
      redirectTo,
    });
    if (typeof window !== 'undefined') {
      window.location.assign(url);
      return;
    }
    oauthInFlight = false;
    authState.update((s) => ({ ...s, loading: false }));
  } catch (error) {
    oauthInFlight = false;
    authState.update((s) => ({ ...s, loading: false, error: errorMessage(error) }));
  }
}

export async function signOut() {
  if (!nhost) return;
  authState.update((s) => ({ ...s, loading: true }));
  try {
    const refreshToken = nhost.getUserSession()?.refreshToken;
    await nhost.auth.signOut({ refreshToken });
    nhost.clearSession();
    authState.update((s) => ({
      ...s,
      loading: false,
      error: null,
      session: null,
      user: null,
    }));
  } catch (error) {
    nhost.clearSession();
    authState.update((s) => ({
      ...s,
      loading: false,
      error: errorMessage(error),
      session: null,
      user: null,
    }));
  }
}

export function disposeAuth() {
  if (unlistenAuthChange) {
    unlistenAuthChange();
    unlistenAuthChange = null;
  }
}
