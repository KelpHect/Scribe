export type AppTheme = 'scribe' | 'neutral' | 'dark';

const validThemes = new Set<AppTheme>(['scribe', 'neutral', 'dark']);

export function normalizeTheme(theme: string | null | undefined): AppTheme {
  return theme && validThemes.has(theme as AppTheme) ? (theme as AppTheme) : 'scribe';
}

export function applyTheme(theme: string | null | undefined): AppTheme {
  const nextTheme = normalizeTheme(theme);
  document.documentElement.dataset.theme = nextTheme;
  return nextTheme;
}
