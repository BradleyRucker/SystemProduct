type Theme = 'dark' | 'light';

const THEME_KEY = 'systemproduct.theme';

export function getPreferredTheme(): Theme {
  if (typeof localStorage !== 'undefined') {
    const stored = localStorage.getItem(THEME_KEY) as Theme | null;
    if (stored === 'dark' || stored === 'light') return stored;
  }
  if (typeof window !== 'undefined' && window.matchMedia) {
    return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  }
  return 'dark';
}

export function applyTheme(theme: Theme) {
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-theme', theme);
  }
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(THEME_KEY, theme);
  }
}

export function toggleTheme(current?: Theme): Theme {
  const next: Theme = (current ?? getPreferredTheme()) === 'dark' ? 'light' : 'dark';
  applyTheme(next);
  return next;
}

export function getTheme(): Theme {
  if (typeof document !== 'undefined') {
    const t = document.documentElement.getAttribute('data-theme') as Theme | null;
    if (t === 'dark' || t === 'light') return t;
  }
  return getPreferredTheme();
}
