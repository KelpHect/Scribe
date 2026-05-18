import type { QueryClient } from '@tanstack/query-core';
import type { RemoteAddonDetails } from '$lib/services/esoui-service';

export const addonDetailsQueryRoot = ['addon-details'] as const;
export const ADDON_DETAIL_STALE_MS = 5 * 60 * 1000;
export const ADDON_DETAIL_GC_MS = 10 * 60 * 1000;
export const ADDON_DETAIL_MAX_QUERIES = 24;
export const ADDON_DETAIL_MAX_SCREENSHOTS = 12;

type AddonDetailQuery = {
  queryKey: readonly unknown[];
  state: {
    data: unknown;
    dataUpdatedAt: number;
  };
};

export function addonDetailsQueryKey(uid: string) {
  return [...addonDetailsQueryRoot, uid] as const;
}

export function trimAddonDetailQueries(queryClient: QueryClient, keepUID = ''): void {
  const queries = getAddonDetailQueries(queryClient);
  if (queries.length <= ADDON_DETAIL_MAX_QUERIES) return;

  const removable = queries
    .filter((query) => getAddonDetailUID(query) !== keepUID)
    .sort((a, b) => a.state.dataUpdatedAt - b.state.dataUpdatedAt);

  const removeCount = queries.length - ADDON_DETAIL_MAX_QUERIES;
  for (const query of removable.slice(0, removeCount)) {
    queryClient.removeQueries({ queryKey: query.queryKey, exact: true });
  }
}

export function getAddonDetailCacheStats(queryClient: QueryClient, now = Date.now()) {
  const queries = getAddonDetailQueries(queryClient);
  const cachedUIDs: string[] = [];
  const screenshotUrls = new Set<string>();
  let withData = 0;
  let fresh = 0;
  let stale = 0;

  for (const query of queries) {
    const data = query.state.data;
    const hasData = data !== undefined && data !== null;
    if (!hasData) continue;

    withData++;
    const age = now - query.state.dataUpdatedAt;
    if (age <= ADDON_DETAIL_STALE_MS) {
      fresh++;
    } else {
      stale++;
    }

    const uid = getAddonDetailUID(query);
    if (uid) cachedUIDs.push(uid);

    const details = data as RemoteAddonDetails;
    for (const url of details.uiIMGs ?? []) screenshotUrls.add(url);
    for (const url of details.uiIMGThumbs ?? []) screenshotUrls.add(url);
  }

  return {
    totalQueries: queries.length,
    queriesWithData: withData,
    fresh,
    stale,
    cachedUIDs: cachedUIDs.slice(0, 8),
    screenshotUrls: screenshotUrls.size,
    maxQueries: ADDON_DETAIL_MAX_QUERIES,
    maxScreenshotsPerDetail: ADDON_DETAIL_MAX_SCREENSHOTS
  };
}

function getAddonDetailQueries(queryClient: QueryClient): AddonDetailQuery[] {
  return queryClient.getQueryCache().findAll({ queryKey: addonDetailsQueryRoot });
}

function getAddonDetailUID(query: AddonDetailQuery): string {
  return typeof query.queryKey[1] === 'string' ? query.queryKey[1] : '';
}
