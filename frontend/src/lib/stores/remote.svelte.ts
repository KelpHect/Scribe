import { fetchAddonDetails, type RemoteAddonDetails } from '$lib/services/esoui-service';
import { refreshRemoteCatalog } from '$lib/db/query-state';
import {
  installAddon,
  batchInstall as batchInstallFn,
  isInstallActive
} from '$lib/stores/downloads.svelte';
import { filterNewInstallUIDs } from '$lib/stores/install-queue';
import { SvelteSet } from 'svelte/reactivity';

let searchQuery: string = $state('');
let sortBy: 'title' | 'author' | 'category' | 'downloads' | 'favorites' | 'date' =
  $state('downloads');
let sortDirection: 'asc' | 'desc' = $state('desc');
let categoryFilter: string[] = $state([]);
let hideInstalled: boolean = $state(true);

function normalizeCategoryFilter(value: string | string[]): string[] {
  const values = (Array.isArray(value) ? value : value.split(','))
    .map((item) => item.trim())
    .filter(Boolean);
  return Array.from(new Set(values));
}

export function serializeCategoryFilter(value: string[]): string {
  return value.join(',');
}

export function parseSortValue(value: string | null | undefined): {
  sortBy: 'title' | 'author' | 'category' | 'downloads' | 'favorites' | 'date';
  sortDirection: 'asc' | 'desc';
} {
  const [fieldRaw, directionRaw] = (value || '').split(':');
  const field =
    fieldRaw === 'title' ||
    fieldRaw === 'author' ||
    fieldRaw === 'category' ||
    fieldRaw === 'downloads' ||
    fieldRaw === 'favorites' ||
    fieldRaw === 'date'
      ? fieldRaw
      : 'downloads';
  const direction = directionRaw === 'asc' ? 'asc' : 'desc';
  return { sortBy: field, sortDirection: direction };
}

export function serializeSortValue(
  sortField: 'title' | 'author' | 'category' | 'downloads' | 'favorites' | 'date',
  direction: 'asc' | 'desc'
): string {
  return `${sortField}:${direction}`;
}

let installing: boolean = $state(false);
let installingUID: string | null = $state(null);
const pendingInstallUIDs = new SvelteSet<string>();
let installError: string | null = $state(null);
let refreshing: boolean = $state(false);

// sidebar needs a tiny bit of shared state here instead of subscribing to the whole installed query
let updateCount: number = $state(0);

export function _setUpdateCount(n: number) {
  updateCount = n;
}

async function forceRefresh(): Promise<void> {
  if (refreshing) return;
  refreshing = true;
  try {
    await refreshRemoteCatalog();
  } finally {
    refreshing = false;
  }
}

async function getDetails(uid: string): Promise<RemoteAddonDetails | null> {
  return fetchAddonDetails(uid);
}

function setSearch(query: string) {
  searchQuery = query;
}

function setSortBy(value: 'title' | 'author' | 'category' | 'downloads' | 'favorites' | 'date') {
  sortBy = value;
}

function setSortDirection(value: 'asc' | 'desc') {
  sortDirection = value;
}

function setCategoryFilter(value: string | string[]) {
  categoryFilter = normalizeCategoryFilter(value);
}

function setHideInstalled(value: boolean) {
  hideInstalled = value;
}

function isInstallingUID(uid: string): boolean {
  return pendingInstallUIDs.has(uid) || isInstallActive(uid);
}

function firstPendingUID(): string | null {
  for (const uid of pendingInstallUIDs) return uid;
  return null;
}

async function install(uid: string): Promise<boolean> {
  const normalizedUID = uid.trim();
  if (!normalizedUID || isInstallingUID(normalizedUID)) return false;
  pendingInstallUIDs.add(normalizedUID);
  installing = pendingInstallUIDs.size > 0;
  installingUID = normalizedUID;
  installError = null;
  try {
    return await installAddon(normalizedUID);
  } catch (e) {
    installError = e instanceof Error ? e.message : `Install failed for UID ${normalizedUID}`;
    throw e;
  } finally {
    pendingInstallUIDs.delete(normalizedUID);
    installing = pendingInstallUIDs.size > 0;
    installingUID = firstPendingUID();
  }
}

async function batchInstall(uids: string[]): Promise<number> {
  const uniqueUIDs = filterNewInstallUIDs(uids, isInstallingUID);
  if (uniqueUIDs.length === 0) return 0;
  for (const uid of uniqueUIDs) {
    pendingInstallUIDs.add(uid);
  }
  installing = pendingInstallUIDs.size > 0;
  installingUID = firstPendingUID();
  installError = null;
  try {
    return await batchInstallFn(uniqueUIDs);
  } catch (e) {
    installError = e instanceof Error ? e.message : 'Batch install failed';
    throw e;
  } finally {
    for (const uid of uniqueUIDs) {
      pendingInstallUIDs.delete(uid);
    }
    installing = pendingInstallUIDs.size > 0;
    installingUID = firstPendingUID();
  }
}

export function getRemoteStore() {
  return {
    get searchQuery() {
      return searchQuery;
    },
    get sortBy() {
      return sortBy;
    },
    get sortDirection() {
      return sortDirection;
    },
    get categoryFilter() {
      return categoryFilter;
    },
    get hideInstalled() {
      return hideInstalled;
    },
    get installing() {
      return installing;
    },
    get installingUID() {
      return installingUID;
    },
    isInstallingUID,
    get installError() {
      return installError;
    },
    get refreshing() {
      return refreshing;
    },
    get updateCount() {
      return updateCount;
    },
    forceRefresh,
    getDetails,
    setSearch,
    setSortBy,
    setSortDirection,
    setCategoryFilter,
    setHideInstalled,
    install,
    batchInstall
  };
}
