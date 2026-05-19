import type { RemoteCatalogStatus } from '$lib/services/esoui-service';

export type CatalogStatusView =
  | 'ready'
  | 'refreshing-cache'
  | 'stale-refresh-failed'
  | 'showing-stale-cache'
  | 'no-cache';

interface CatalogStatusInput {
  remoteCount: number;
  hasRemoteCatalogStatus: boolean;
  remoteStatus: RemoteCatalogStatus | null;
  isError: boolean;
}

export function getCatalogStatusView(input: CatalogStatusInput): CatalogStatusView {
  if (input.remoteCount > 0 && input.remoteStatus?.refreshInFlight) {
    return 'refreshing-cache';
  }
  if (
    input.remoteCount > 0 &&
    input.remoteStatus?.cacheStale &&
    input.remoteStatus.lastRefreshError
  ) {
    return 'stale-refresh-failed';
  }
  if (
    input.remoteCount > 0 &&
    input.hasRemoteCatalogStatus &&
    input.remoteStatus?.cacheStale &&
    !input.remoteStatus.lastRefreshError
  ) {
    return 'showing-stale-cache';
  }
  if (
    input.remoteCount === 0 &&
    (input.isError || (input.hasRemoteCatalogStatus && !input.remoteStatus?.hasData))
  ) {
    return 'no-cache';
  }
  return 'ready';
}
