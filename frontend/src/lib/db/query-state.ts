import { fetchAddonPath, fetchInstalledAddons, type Addon } from '$lib/services/addon-service';
import {
  fetchCategories,
  fetchMatchedAddons,
  fetchRemoteAddons,
  refreshRemoteAddons,
  type Category,
  type MatchedAddon,
  type RemoteAddon
} from '$lib/services/esoui-service';
import { queryClient } from './client';

export const installedAddonsQueryKey = ['installed-addons'] as const;
export const addonPathQueryKey = ['addon-path'] as const;
export const remoteAddonsQueryKey = ['remote-addons'] as const;
export const categoriesQueryKey = ['categories'] as const;
export const matchedAddonsQueryKey = ['matched-addons'] as const;

export async function refreshRemoteCatalog(): Promise<void> {
  const addons = await refreshRemoteAddons();
  queryClient.setQueryData(remoteAddonsQueryKey, addons);

  const categoriesState = queryClient.getQueryState(categoriesQueryKey);
  if (!categoriesState?.dataUpdatedAt) {
    const categories = await fetchCategories();
    queryClient.setQueryData(categoriesQueryKey, categories);
  }

  await queryClient.refetchQueries({ queryKey: matchedAddonsQueryKey, exact: true });
}

export function refreshInstalledState(): Promise<void> {
  return Promise.all([
    queryClient.refetchQueries({ queryKey: installedAddonsQueryKey, exact: true }),
    queryClient.refetchQueries({ queryKey: addonPathQueryKey, exact: true }),
    queryClient.refetchQueries({ queryKey: matchedAddonsQueryKey, exact: true })
  ]).then(() => undefined);
}

export function fetchAddonPathQuery(): Promise<string> {
  return queryClient.fetchQuery({
    queryKey: addonPathQueryKey,
    queryFn: async (): Promise<string> => fetchAddonPath()
  });
}

export function remoteAddonsQueryFn(): Promise<RemoteAddon[]> {
  return fetchRemoteAddons();
}

export function matchedAddonsQueryFn(): Promise<MatchedAddon[]> {
  return fetchMatchedAddons();
}

export function categoriesQueryFn(): Promise<Category[]> {
  return fetchCategories();
}

export function installedAddonsQueryFn(): Promise<Addon[]> {
  return fetchInstalledAddons();
}
