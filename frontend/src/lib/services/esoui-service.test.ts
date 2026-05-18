import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fetchMatchedAddons, fetchMissingDependencies, fetchRemoteCatalogStatus } from './esoui-service';
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

describe('fetchMissingDependencies', () => {
  beforeEach(() => {
    mockedCallWails.mockReset();
  });

  it('normalizes dependency plan fields for confirmation flows', async () => {
    mockedCallWails.mockResolvedValueOnce([
      {
        depFolderName: 'LibNeeded',
        requiredBy: null,
        remoteUID: '123',
        remoteName: 'Lib Needed',
        canInstall: true,
        optional: false
      }
    ] as never);

    await expect(fetchMissingDependencies()).resolves.toEqual([
      expect.objectContaining({
        depFolderName: 'LibNeeded',
        requiredBy: [],
        versionConstraints: [],
        planState: 'installable',
        planReason: 'Matched ESOUI addon metadata and can be queued for install.'
      })
    ]);
  });
});

describe('fetchMatchedAddons', () => {
  beforeEach(() => {
    mockedCallWails.mockReset();
  });

  it('normalizes stale generated bindings for update decision flows', async () => {
    mockedCallWails.mockResolvedValueOnce([
      {
        folderName: 'NeedsUpdate',
        remote: null,
        details: undefined,
        updateAvailable: true,
        localVersion: '1',
        remoteVersion: '2'
      }
    ] as never);

    await expect(fetchMatchedAddons()).resolves.toEqual([
      expect.objectContaining({
        folderName: 'NeedsUpdate',
        remote: null,
        details: null,
        updateState: 'remote-newer',
        updateReason: ''
      })
    ]);
  });
});
