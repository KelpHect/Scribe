import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fetchRemoteCatalogStatus } from './esoui-service';
import { callWails } from './wails-service';

vi.mock('./wails-service', () => ({
  callWails: vi.fn()
}));

const mockedCallWails = vi.mocked(callWails);

describe('fetchRemoteCatalogStatus', () => {
  beforeEach(() => {
    mockedCallWails.mockReset();
  });

  it('returns the Wails catalog status payload', async () => {
    mockedCallWails.mockResolvedValueOnce({
      hasData: true,
      cacheStale: true,
      lastRefreshError: 'network unavailable'
    } as never);

    await expect(fetchRemoteCatalogStatus()).resolves.toEqual({
      hasData: true,
      cacheStale: true,
      lastRefreshError: 'network unavailable'
    });
    expect(mockedCallWails).toHaveBeenCalledWith('GetRemoteCatalogStatus');
  });

  it('falls back to an empty status when Wails returns no payload', async () => {
    mockedCallWails.mockResolvedValueOnce(undefined as never);

    await expect(fetchRemoteCatalogStatus()).resolves.toEqual({
      hasData: false,
      cacheStale: false,
      lastRefreshError: ''
    });
  });
});
