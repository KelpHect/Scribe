import { describe, expect, it } from 'vitest';
import { getCatalogStatusView } from './status';

describe('getCatalogStatusView', () => {
  it('keeps cached results visible when a background refresh fails', () => {
    expect(
      getCatalogStatusView({
        remoteCount: 25,
        hasRemoteCatalogStatus: true,
        remoteStatus: { hasData: true, cacheStale: true, lastRefreshError: 'offline' },
        isError: false
      })
    ).toBe('stale-refresh-failed');
  });

  it('marks stale cached data as visible while refresh continues', () => {
    expect(
      getCatalogStatusView({
        remoteCount: 25,
        hasRemoteCatalogStatus: true,
        remoteStatus: { hasData: true, cacheStale: true, lastRefreshError: '' },
        isError: false
      })
    ).toBe('showing-stale-cache');
  });

  it('distinguishes no cached data from an empty filtered result', () => {
    expect(
      getCatalogStatusView({
        remoteCount: 0,
        hasRemoteCatalogStatus: true,
        remoteStatus: { hasData: false, cacheStale: false, lastRefreshError: '' },
        isError: false
      })
    ).toBe('no-cache');
  });

  it('uses ready for ordinary loaded or filtered-empty states', () => {
    expect(
      getCatalogStatusView({
        remoteCount: 0,
        hasRemoteCatalogStatus: true,
        remoteStatus: { hasData: true, cacheStale: false, lastRefreshError: '' },
        isError: false
      })
    ).toBe('ready');
  });
});
