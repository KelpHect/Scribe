import { compareVersionStrings } from '$lib/utils';
import type { GameVersion, RemoteAddon } from '$lib/services/esoui-service';

export function getLatestCompatibility(
  compatibilities: GameVersion[] | null | undefined
): GameVersion | null {
  let latest: GameVersion | null = null;

  for (const compatibility of compatibilities ?? []) {
    if (!compatibility.version) continue;
    if (!latest || compareVersionStrings(compatibility.version, latest.version) > 0) {
      latest = compatibility;
    }
  }

  return latest;
}

export function remoteAddonSearchScore(addon: RemoteAddon, query: string): number {
  const q = query.toLowerCase().trim();
  if (!q) return 0;

  const name = addon.uiName.toLowerCase();
  const dirs = (addon.uiDirs ?? []).map((dir) => dir.toLowerCase());
  const author = addon.uiAuthorName.toLowerCase();

  if (name === q || dirs.some((dir) => dir === q)) return 0;
  if (name.startsWith(q) || dirs.some((dir) => dir.startsWith(q))) return 1;
  if (name.includes(q) || dirs.some((dir) => dir.includes(q))) return 2;
  if (author.includes(q)) return 3;
  return Number.POSITIVE_INFINITY;
}

export function isLibraryLikeRemoteAddon(addon: RemoteAddon, categoryName = ''): boolean {
  const haystack =
    `${addon.uiName} ${categoryName} ${(addon.uiDirs ?? []).join(' ')}`.toLowerCase();
  return /\blib|library|libraries|dependency|dependencies/.test(haystack);
}
