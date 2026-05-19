import { QueryClient } from '@tanstack/query-core';

export const DEFAULT_QUERY_STALE_MS = 5 * 60 * 1000;
export const DEFAULT_QUERY_GC_MS = 60 * 60 * 1000;
export const REMOTE_CATALOG_STALE_MS = 30 * 60 * 1000;
export const REMOTE_CATALOG_GC_MS = 2 * 60 * 60 * 1000;
export const REMOTE_STATUS_STALE_MS = 30 * 1000;
export const REMOTE_STATUS_GC_MS = 30 * 60 * 1000;

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: DEFAULT_QUERY_STALE_MS,
      gcTime: DEFAULT_QUERY_GC_MS,
      refetchOnWindowFocus: false
    }
  }
});
