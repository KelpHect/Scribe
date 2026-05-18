import { describe, expect, it } from 'vitest';
import { getLatestCompatibility, isLibraryLikeRemoteAddon, remoteAddonSearchScore } from './remote-list';
import type { RemoteAddon } from '$lib/services/esoui-service';

describe('getLatestCompatibility', () => {
  it('selects the latest version without mutating the source list', () => {
    const versions = [
      { version: '101041', name: 'Older' },
      { version: '101046', name: 'Current' },
      { version: '101042', name: 'Middle' }
    ];

    expect(getLatestCompatibility(versions)).toEqual({ version: '101046', name: 'Current' });
    expect(versions.map((v) => v.version)).toEqual(['101041', '101046', '101042']);
  });
});

describe('remoteAddonSearchScore', () => {
  const base = {
    uid: '1',
    categoryId: 'cat',
    uiAuthorName: 'Author',
    uiDate: '',
    uiVersion: '',
    uiFileInfoUrl: '',
    uiDownloadTotal: 0,
    uiFavoriteTotal: 0,
    uiIMGThumbs: [],
    uiIMGs: [],
    compatabilities: [],
    siblings: []
  } satisfies Omit<RemoteAddon, 'uiName' | 'uiDirs'>;

  it('ranks exact title and folder matches ahead of loose author matches', () => {
    expect(remoteAddonSearchScore({ ...base, uiName: 'LibFoo', uiDirs: ['LibFoo'] }, 'libfoo')).toBe(0);
    expect(remoteAddonSearchScore({ ...base, uiName: 'LibFoo Extras', uiDirs: ['LibFooExtras'] }, 'libfoo')).toBe(1);
    expect(remoteAddonSearchScore({ ...base, uiName: 'Better LibFoo', uiDirs: ['BetterLibFoo'] }, 'libfoo')).toBe(2);
    expect(remoteAddonSearchScore({ ...base, uiName: 'Other', uiAuthorName: 'LibFoo Team', uiDirs: ['Other'] }, 'libfoo')).toBe(3);
  });

  it('identifies library-like catalog entries for dependency filtering', () => {
    expect(isLibraryLikeRemoteAddon({ ...base, uiName: 'LibAddonMenu', uiDirs: ['LibAddonMenu'] })).toBe(true);
    expect(isLibraryLikeRemoteAddon({ ...base, uiName: 'Quest Pins', uiDirs: ['QuestPins'] })).toBe(false);
    expect(isLibraryLikeRemoteAddon({ ...base, uiName: 'Helper', uiDirs: ['Helper'] }, 'Libraries')).toBe(true);
  });
});
