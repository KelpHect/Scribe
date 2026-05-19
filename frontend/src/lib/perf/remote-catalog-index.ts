import { compareEsoUiCategoryOrder, compareVersionStrings, getUpdatedState } from '$lib/utils';
import type { Category, RemoteAddon } from '$lib/services/esoui-service';
import { getLatestCompatibility, isLibraryLikeRemoteAddon } from './remote-list';

export type RemoteSortBy = 'title' | 'author' | 'category' | 'downloads' | 'favorites' | 'date';
export type RemoteSortDirection = 'asc' | 'desc';
export type RemoteContentFilter = 'all' | 'libraries';

export type IndexedRemoteAddon = {
  addon: RemoteAddon;
  nameLower: string;
  authorLower: string;
  dirsLower: string[];
  category: Category | null;
  categoryName: string;
  categoryNameLower: string;
  categoryOrder: number;
  libraryLike: boolean;
  listIconUrl?: string;
  listIconIsThumbnail: boolean;
  compatibilityVersions: string[];
  latestCompatibilityVersion: string;
  latestCompatibilityName: string;
  updatedState: ReturnType<typeof getUpdatedState>;
  dateTime: number;
};

export type RemoteCatalogFilterOptions = {
  query: string;
  contentFilter: RemoteContentFilter;
  hideInstalled: boolean;
  installedUIDs: ReadonlySet<string>;
  versionFilter: string;
  categoryFilter: readonly string[];
  sortKey: RemoteSortBy;
  sortDirection: RemoteSortDirection;
};

export type RemoteCatalogFilterResult = {
  list: IndexedRemoteAddon[];
  countMap: Map<string, number>;
  query: string;
  sortKey: RemoteSortBy;
  sortDirection: RemoteSortDirection;
};

type ScoredIndexedRemoteAddon = {
  item: IndexedRemoteAddon;
  searchScore: number;
};

export function buildRemoteCatalogIndex(
  addons: readonly RemoteAddon[],
  categories: readonly Category[]
): IndexedRemoteAddon[] {
  const categoryMap = new Map(categories.map((category) => [category.id, category]));
  const categoryOrder = new Map(
    [...categories].sort(compareEsoUiCategoryOrder).map((category, index) => [category.id, index])
  );

  return addons.map((addon) => {
    const category = categoryMap.get(addon.categoryId) ?? null;
    const thumb = addon.uiIMGThumbs?.[0] || addon.uiIMGs?.[0] || category?.iconUrl || undefined;
    const latestCompatibility = getLatestCompatibility(addon.compatabilities);

    return {
      addon,
      nameLower: addon.uiName.toLowerCase(),
      authorLower: addon.uiAuthorName.toLowerCase(),
      dirsLower: (addon.uiDirs ?? []).map((dir) => dir.toLowerCase()),
      category,
      categoryName: category?.name ?? '',
      categoryNameLower: (category?.name ?? '').toLowerCase(),
      categoryOrder: categoryOrder.get(addon.categoryId) ?? Number.MAX_SAFE_INTEGER,
      libraryLike: isLibraryLikeRemoteAddon(addon, category?.name ?? ''),
      listIconUrl: thumb,
      listIconIsThumbnail: !!(addon.uiIMGThumbs?.[0] || addon.uiIMGs?.[0]),
      compatibilityVersions: (addon.compatabilities ?? []).map((cv) => cv.version),
      latestCompatibilityVersion: latestCompatibility?.version ?? '',
      latestCompatibilityName: latestCompatibility?.name ?? '',
      updatedState: getUpdatedState(addon.uiDate),
      dateTime: parseCatalogDate(addon.uiDate)
    };
  });
}

export function normalizeRemoteSearchQuery(query: string): string {
  return query.toLowerCase().trim();
}

export function scoreIndexedRemoteAddon(
  addon: IndexedRemoteAddon,
  normalizedQuery: string
): number {
  const q = normalizedQuery.trim();
  if (!q) return 0;

  if (addon.nameLower === q || addon.dirsLower.some((dir) => dir === q)) return 0;
  if (addon.nameLower.startsWith(q) || addon.dirsLower.some((dir) => dir.startsWith(q))) return 1;
  if (addon.nameLower.includes(q) || addon.dirsLower.some((dir) => dir.includes(q))) return 2;
  if (addon.authorLower.includes(q)) return 3;
  return Number.POSITIVE_INFINITY;
}

export function filterRemoteCatalog(
  index: readonly IndexedRemoteAddon[],
  options: RemoteCatalogFilterOptions
): RemoteCatalogFilterResult {
  const query = normalizeRemoteSearchQuery(options.query);
  const selectedCategories = new Set(options.categoryFilter);
  const countMap = new Map<string, number>();
  const scored: ScoredIndexedRemoteAddon[] = [];

  for (const item of index) {
    const searchScore = scoreIndexedRemoteAddon(item, query);
    if (query && !Number.isFinite(searchScore)) continue;
    if (options.contentFilter === 'libraries' && !item.libraryLike) continue;
    if (options.hideInstalled && options.installedUIDs.has(item.addon.uid)) continue;
    if (options.versionFilter && !item.compatibilityVersions.includes(options.versionFilter))
      continue;

    countMap.set(item.addon.categoryId, (countMap.get(item.addon.categoryId) ?? 0) + 1);

    if (selectedCategories.size > 0 && !selectedCategories.has(item.addon.categoryId)) continue;
    scored.push({ item, searchScore });
  }

  scored.sort((a, b) => compareScored(a, b, query, options.sortKey, options.sortDirection));

  return {
    list: scored.map((entry) => entry.item),
    countMap,
    query,
    sortKey: options.sortKey,
    sortDirection: options.sortDirection
  };
}

function compareScored(
  a: ScoredIndexedRemoteAddon,
  b: ScoredIndexedRemoteAddon,
  query: string,
  sortKey: RemoteSortBy,
  sortDirection: RemoteSortDirection
): number {
  if (query) {
    const score = a.searchScore - b.searchScore;
    if (score !== 0) return score;
  }

  const result = compareIndexedRemoteAddon(a.item, b.item, sortKey);
  return sortDirection === 'asc' ? result : -result;
}

function compareIndexedRemoteAddon(
  a: IndexedRemoteAddon,
  b: IndexedRemoteAddon,
  sortKey: RemoteSortBy
): number {
  switch (sortKey) {
    case 'downloads':
      return numericThenTitle(a.addon.uiDownloadTotal ?? 0, b.addon.uiDownloadTotal ?? 0, a, b);
    case 'favorites':
      return numericThenTitle(a.addon.uiFavoriteTotal ?? 0, b.addon.uiFavoriteTotal ?? 0, a, b);
    case 'date':
      return numericThenTitle(a.dateTime, b.dateTime, a, b);
    case 'author':
      return (
        compareStrings(a.authorLower, b.authorLower) || compareStrings(a.nameLower, b.nameLower)
      );
    case 'category':
      return (
        a.categoryOrder - b.categoryOrder ||
        compareStrings(a.categoryNameLower, b.categoryNameLower) ||
        compareStrings(a.nameLower, b.nameLower)
      );
    case 'title':
    default:
      return compareStrings(a.nameLower, b.nameLower);
  }
}

function numericThenTitle(
  aValue: number,
  bValue: number,
  a: IndexedRemoteAddon,
  b: IndexedRemoteAddon
): number {
  return aValue - bValue || compareStrings(a.nameLower, b.nameLower);
}

function compareStrings(a: string, b: string): number {
  return a.localeCompare(b);
}

function parseCatalogDate(value: string): number {
  if (!value) return 0;

  const parsed = Date.parse(value);
  if (Number.isFinite(parsed)) return parsed;

  const normalized = value.match(/^(\d{1,2})\/(\d{1,2})\/(\d{2,4})$/);
  if (!normalized) return 0;

  const [, month, day, year] = normalized;
  const fullYear = year.length === 2 ? `20${year}` : year;
  const fallback = Date.parse(`${fullYear}-${month.padStart(2, '0')}-${day.padStart(2, '0')}`);
  return Number.isFinite(fallback) ? fallback : 0;
}

export function sortGameVersionsDescending(
  versions: Iterable<[version: string, name: string]>
): { version: string; name: string }[] {
  return Array.from(versions)
    .map(([version, name]) => ({ version, name }))
    .sort((a, b) => compareVersionStrings(b.version, a.version));
}
