import { describeUpdateAction } from '$lib/addons/decision';
import type { Addon } from '$lib/services/addon-service';
import type { Category, MatchedAddon, RemoteAddon } from '$lib/services/esoui-service';

export const INSTALLED_UNCATEGORIZED_ID = '__uncategorized__';
export const INSTALLED_LIBRARIES_ID = '__libraries__';

export type IndexedInstalledAddon = {
  addon: Addon;
  folderLower: string;
  titleLower: string;
  authorLower: string;
  matched: MatchedAddon | null;
  category: Category | null;
  categoryIconUrl: string;
  listIconUrl?: string;
  listIconIsThumbnail: boolean;
  hasUpdate: boolean;
  updateAction: ReturnType<typeof describeUpdateAction>;
};

export type InstalledAddonIndex = {
  addons: IndexedInstalledAddon[];
  matchedMap: Map<string, MatchedAddon>;
  categoryMap: Map<string, Category>;
  installedFolderNames: Set<string>;
  libraryCount: number;
  totalCount: number;
  updatesAvailable: MatchedAddon[];
  updateSet: Set<string>;
  libraryCategoryIconUrl: string;
};

export type InstalledCategoryGroup = {
  id: string;
  name: string;
  iconUrl: string;
  addons: IndexedInstalledAddon[];
};

export type InstalledFlatRow =
  | { type: 'header'; group: InstalledCategoryGroup }
  | { type: 'addon'; item: IndexedInstalledAddon; groupId: string };

export function buildInstalledAddonIndex(
  addons: readonly Addon[],
  matchedAddons: readonly MatchedAddon[],
  categories: readonly Category[]
): InstalledAddonIndex {
  const matchedMap = new Map<string, MatchedAddon>();
  const categoryMap = new Map<string, Category>();
  const installedFolderNames = new Set<string>();
  const updatesAvailable: MatchedAddon[] = [];
  const updateSet = new Set<string>();
  let libraryCount = 0;

  for (const match of matchedAddons) {
    const key = match.folderName.toLowerCase();
    matchedMap.set(key, match);
    if (match.updateAvailable) {
      updatesAvailable.push(match);
      updateSet.add(key);
    }
  }

  for (const category of categories) {
    categoryMap.set(category.id, category);
  }

  const indexed = addons.map((addon) => {
    const folderLower = addon.folderName.toLowerCase();
    const matched = matchedMap.get(folderLower) ?? null;
    const category = matched?.remote?.categoryId
      ? (categoryMap.get(matched.remote.categoryId) ?? null)
      : null;
    const categoryIconUrl = category?.iconUrl ?? '';
    const listImageUrl = addon.isLibrary ? '' : (matched?.remote?.uiIMGs?.[0] ?? '');
    const updateAction = describeUpdateAction({
      installed: true,
      updateAvailable: matched?.updateAvailable ?? false,
      updateState: matched?.updateState ?? (matched?.remote ? 'up-to-date' : 'unmatched'),
      updateReason: matched?.updateReason ?? '',
      localVersion: matched?.localVersion ?? addon.version,
      remoteVersion: matched?.remoteVersion ?? matched?.remote?.uiVersion,
      folderName: addon.folderName
    });

    installedFolderNames.add(folderLower);
    if (addon.isLibrary) libraryCount += 1;

    return {
      addon,
      folderLower,
      titleLower: addon.title.toLowerCase(),
      authorLower: addon.author.toLowerCase(),
      matched,
      category,
      categoryIconUrl,
      listIconUrl: listImageUrl || categoryIconUrl || undefined,
      listIconIsThumbnail: !!listImageUrl,
      hasUpdate: updateSet.has(folderLower),
      updateAction
    };
  });

  return {
    addons: indexed,
    matchedMap,
    categoryMap,
    installedFolderNames,
    libraryCount,
    totalCount: addons.length,
    updatesAvailable,
    updateSet,
    libraryCategoryIconUrl: findLibraryCategoryIcon(indexed, categories)
  };
}

export function filterInstalledAddons(
  index: readonly IndexedInstalledAddon[],
  query: string
): IndexedInstalledAddon[] {
  const normalized = query.toLowerCase().trim();
  if (!normalized) return [...index];

  const out: IndexedInstalledAddon[] = [];
  for (const item of index) {
    if (
      item.titleLower.includes(normalized) ||
      item.authorLower.includes(normalized) ||
      item.folderLower.includes(normalized)
    ) {
      out.push(item);
    }
  }
  return out;
}

export function groupInstalledAddons(
  items: readonly IndexedInstalledAddon[],
  libraryCategoryIconUrl = ''
): InstalledCategoryGroup[] {
  const groupMap = new Map<string, IndexedInstalledAddon[]>();
  const libraries: IndexedInstalledAddon[] = [];

  for (const item of items) {
    if (item.addon.isLibrary) {
      libraries.push(item);
      continue;
    }

    const categoryId = item.category?.id ?? INSTALLED_UNCATEGORIZED_ID;
    const existing = groupMap.get(categoryId);
    if (existing) {
      existing.push(item);
    } else {
      groupMap.set(categoryId, [item]);
    }
  }

  const sortAddons = (a: IndexedInstalledAddon, b: IndexedInstalledAddon) =>
    a.titleLower.localeCompare(b.titleLower);
  const namedGroups: InstalledCategoryGroup[] = [];

  for (const [categoryId, addons] of groupMap) {
    if (categoryId === INSTALLED_UNCATEGORIZED_ID) continue;
    const category = addons[0]?.category;
    if (!category) continue;
    namedGroups.push({
      id: categoryId,
      name: category.name,
      iconUrl: category.iconUrl,
      addons: addons.sort(sortAddons)
    });
  }
  namedGroups.sort((a, b) => a.name.localeCompare(b.name));

  const groups = [...namedGroups];
  const uncategorized = groupMap.get(INSTALLED_UNCATEGORIZED_ID);
  if (uncategorized && uncategorized.length > 0) {
    groups.push({
      id: INSTALLED_UNCATEGORIZED_ID,
      name: 'Uncategorized',
      iconUrl: '',
      addons: uncategorized.sort(sortAddons)
    });
  }

  if (libraries.length > 0) {
    groups.push({
      id: INSTALLED_LIBRARIES_ID,
      name: 'Libraries',
      iconUrl: libraryCategoryIconUrl,
      addons: libraries.sort(sortAddons)
    });
  }

  return groups;
}

export function flattenInstalledGroups(
  groups: readonly InstalledCategoryGroup[],
  expandedCategories: ReadonlySet<string>
): InstalledFlatRow[] {
  const rows: InstalledFlatRow[] = [];
  for (const group of groups) {
    rows.push({ type: 'header', group });
    if (!expandedCategories.has(group.id)) continue;
    for (const item of group.addons) {
      rows.push({ type: 'addon', item, groupId: group.id });
    }
  }
  return rows;
}

export function buildRemoteDependencyUIDMap(remotes: readonly RemoteAddon[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const remote of remotes) {
    for (const dir of remote.uiDirs ?? []) {
      const key = dir.toLowerCase();
      if (!map.has(key)) map.set(key, remote.uid);
    }
    if (remote.uiName) {
      const key = remote.uiName.toLowerCase();
      if (!map.has(key)) map.set(key, remote.uid);
    }
  }
  return map;
}

function findLibraryCategoryIcon(
  indexed: readonly IndexedInstalledAddon[],
  categories: readonly Category[]
): string {
  for (const item of indexed) {
    if (item.addon.isLibrary && item.categoryIconUrl) return item.categoryIconUrl;
  }
  return categories.find((category) => /lib/i.test(category.name))?.iconUrl ?? '';
}
