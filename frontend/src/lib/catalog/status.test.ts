import { describe, expect, it } from 'vitest';
import { getCatalogStatusView } from './status';

describe('getCatalogStatusView', () => {
  it('keeps cached results visible when a background refresh fails', () => {
    expect(
      getCatalogStatusView({
        remoteCount: 25,
        hasRemoteCatalogStatus: true,
        remoteStatus: {
          hasData: true,
          cacheStale: true,
          lastRefreshError: 'offline',
          refreshInFlight: false,
          refreshStartedAt: ''
        },
        isError: false
      })
    ).toBe('stale-refresh-failed');
  });

  it('marks stale cached data as visible while refresh continues', () => {
    expect(
      getCatalogStatusView({
        remoteCount: 25,
        hasRemoteCatalogStatus: true,
        remoteStatus: {
          hasData: true,
          cacheStale: true,
          lastRefreshError: '',
          refreshInFlight: false,
          refreshStartedAt: ''
        },
        isError: false
      })
    ).toBe('showing-stale-cache');
  });

  it('marks cached data as visible while a refresh is in flight', () => {
    expect(
      getCatalogStatusView({
        remoteCount: 25,
        hasRemoteCatalogStatus: true,
        remoteStatus: {
          hasData: true,
          cacheStale: true,
          lastRefreshError: '',
          refreshInFlight: true,
          refreshStartedAt: '2026-05-18T12:00:00Z'
        },
        isError: false
      })
    ).toBe('refreshing-cache');
  });

  it('distinguishes no cached data from an empty filtered result', () => {
    expect(
      getCatalogStatusView({
        remoteCount: 0,
        hasRemoteCatalogStatus: true,
        remoteStatus: {
          hasData: false,
          cacheStale: false,
          lastRefreshError: '',
          refreshInFlight: false,
          refreshStartedAt: ''
        },
        isError: false
      })
    ).toBe('no-cache');
  });

  it('uses ready for ordinary loaded or filtered-empty states', () => {
    expect(
      getCatalogStatusView({
        remoteCount: 0,
        hasRemoteCatalogStatus: true,
        remoteStatus: {
          hasData: true,
          cacheStale: false,
          lastRefreshError: '',
          refreshInFlight: false,
          refreshStartedAt: ''
        },
        isError: false
      })
    ).toBe('ready');
  });
});
