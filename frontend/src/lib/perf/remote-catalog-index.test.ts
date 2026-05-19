import { describe, expect, it } from 'vitest';
import {
  buildRemoteCatalogIndex,
  filterRemoteCatalog,
  scoreIndexedRemoteAddon,
  sortGameVersionsDescending
} from './remote-catalog-index';
import type { Category, RemoteAddon } from '$lib/services/esoui-service';

const categories: Category[] = [
  {
    id: 'utility',
    name: 'Utility Mods',
    iconUrl: 'utility.png',
    parentId: '',
    parentIds: [],
    count: 0
  },
  {
    id: 'library',
    name: 'Libraries',
    iconUrl: 'library.png',
    parentId: '',
    parentIds: [],
    count: 0
  },
  {
    id: 'combat',
    name: 'Action Bar Mods',
    iconUrl: 'combat.png',
    parentId: '',
    parentIds: [],
    count: 0
  }
];

const baseAddon = {
  categoryId: 'utility',
  uiAuthorName: 'Author',
  uiDate: '2026-05-01',
  uiVersion: '1.0',
  uiDirs: [],
  uiFileInfoUrl: '',
  uiDownloadTotal: 0,
  uiFavoriteTotal: 0,
  uiIMGThumbs: [],
  uiIMGs: [],
  compatabilities: [],
  siblings: []
} satisfies Omit<RemoteAddon, 'uid' | 'uiName'>;

function addon(input: Partial<RemoteAddon> & Pick<RemoteAddon, 'uid' | 'uiName'>): RemoteAddon {
  return { ...baseAddon, ...input };
}

describe('buildRemoteCatalogIndex', () => {
  it('precomputes stable search, category, image, compatibility, and date fields', () => {
    const index = buildRemoteCatalogIndex(
      [
        addon({
          uid: 'lib',
          uiName: 'LibAddonMenu',
          categoryId: 'library',
          uiAuthorName: 'ESO Team',
          uiDirs: ['LibAddonMenu-2.0'],
          uiIMGThumbs: ['thumb.png'],
          uiIMGs: ['image.png'],
          compatabilities: [
            { version: '101041', name: 'Older' },
            { version: '101046', name: 'Current' }
          ],
          uiDate: '05/18/2026'
        })
      ],
      categories
    );

    expect(index[0]).toMatchObject({
      nameLower: 'libaddonmenu',
      authorLower: 'eso team',
      dirsLower: ['libaddonmenu-2.0'],
      categoryName: 'Libraries',
      categoryNameLower: 'libraries',
      libraryLike: true,
      listIconUrl: 'thumb.png',
      listIconIsThumbnail: true,
      compatibilityVersions: ['101041', '101046'],
      latestCompatibilityVersion: '101046',
      latestCompatibilityName: 'Current'
    });
    expect(index[0].dateTime).toBeGreaterThan(0);
  });
});

describe('filterRemoteCatalog', () => {
  it('reuses indexed search scores so exact and prefix matches outrank loose author matches', () => {
    const index = buildRemoteCatalogIndex(
      [
        addon({ uid: 'author', uiName: 'Other', uiAuthorName: 'LibFoo Team', uiDirs: ['Other'] }),
        addon({ uid: 'prefix', uiName: 'LibFoo Extras', uiDirs: ['LibFooExtras'] }),
        addon({ uid: 'exact', uiName: 'LibFoo', uiDirs: ['LibFoo'] })
      ],
      categories
    );

    expect(scoreIndexedRemoteAddon(index[0], 'libfoo')).toBe(3);

    const result = filterRemoteCatalog(
      index,
      defaultOptions({ query: 'LibFoo', sortKey: 'downloads' })
    );

    expect(result.list.map((item) => item.addon.uid)).toEqual(['exact', 'prefix', 'author']);
  });

  it('counts categories before applying selected category filters', () => {
    const index = buildRemoteCatalogIndex(
      [
        addon({ uid: 'one', uiName: 'Alpha', categoryId: 'utility' }),
        addon({ uid: 'two', uiName: 'Beta', categoryId: 'combat' }),
        addon({ uid: 'three', uiName: 'Gamma', categoryId: 'combat' })
      ],
      categories
    );

    const result = filterRemoteCatalog(
      index,
      defaultOptions({ categoryFilter: ['utility'], sortKey: 'title', sortDirection: 'asc' })
    );

    expect(result.list.map((item) => item.addon.uid)).toEqual(['one']);
    expect(result.countMap.get('utility')).toBe(1);
    expect(result.countMap.get('combat')).toBe(2);
  });

  it('applies installed, content, and game-version filters before category counts', () => {
    const index = buildRemoteCatalogIndex(
      [
        addon({
          uid: 'installed-lib',
          uiName: 'LibInstalled',
          categoryId: 'library',
          compatabilities: [{ version: '101046', name: 'Current' }]
        }),
        addon({
          uid: 'current-lib',
          uiName: 'LibCurrent',
          categoryId: 'library',
          compatabilities: [{ version: '101046', name: 'Current' }]
        }),
        addon({
          uid: 'old-lib',
          uiName: 'LibOld',
          categoryId: 'library',
          compatabilities: [{ version: '101041', name: 'Older' }]
        }),
        addon({
          uid: 'normal-addon',
          uiName: 'Normal Addon',
          categoryId: 'utility',
          compatabilities: [{ version: '101046', name: 'Current' }]
        })
      ],
      categories
    );

    const result = filterRemoteCatalog(
      index,
      defaultOptions({
        contentFilter: 'libraries',
        hideInstalled: true,
        installedUIDs: new Set(['installed-lib']),
        versionFilter: '101046',
        sortKey: 'title',
        sortDirection: 'asc'
      })
    );

    expect(result.list.map((item) => item.addon.uid)).toEqual(['current-lib']);
    expect([...result.countMap.entries()]).toEqual([['library', 1]]);
  });

  it('sorts using precomputed date, number, text, and category keys', () => {
    const index = buildRemoteCatalogIndex(
      [
        addon({
          uid: 'low',
          uiName: 'Alpha',
          uiAuthorName: 'Zed',
          categoryId: 'utility',
          uiDownloadTotal: 10,
          uiFavoriteTotal: 2,
          uiDate: '2026-05-01'
        }),
        addon({
          uid: 'high',
          uiName: 'Beta',
          uiAuthorName: 'Ann',
          categoryId: 'combat',
          uiDownloadTotal: 50,
          uiFavoriteTotal: 9,
          uiDate: '2026-05-18'
        })
      ],
      categories
    );

    expect(
      filterRemoteCatalog(index, defaultOptions({ sortKey: 'downloads' })).list[0].addon.uid
    ).toBe('high');
    expect(
      filterRemoteCatalog(index, defaultOptions({ sortKey: 'favorites' })).list[0].addon.uid
    ).toBe('high');
    expect(filterRemoteCatalog(index, defaultOptions({ sortKey: 'date' })).list[0].addon.uid).toBe(
      'high'
    );
    expect(
      filterRemoteCatalog(index, defaultOptions({ sortKey: 'author', sortDirection: 'asc' }))
        .list[0].addon.uid
    ).toBe('high');
    expect(
      filterRemoteCatalog(index, defaultOptions({ sortKey: 'category', sortDirection: 'asc' }))
        .list[0].addon.uid
    ).toBe('high');
  });
});

describe('sortGameVersionsDescending', () => {
  it('sorts versions once for UI option generation', () => {
    expect(
      sortGameVersionsDescending([
        ['101041', 'Older'],
        ['101046', 'Current'],
        ['101042', 'Middle']
      ]).map((version) => version.version)
    ).toEqual(['101046', '101042', '101041']);
  });
});

function defaultOptions(
  overrides: Partial<Parameters<typeof filterRemoteCatalog>[1]> = {}
): Parameters<typeof filterRemoteCatalog>[1] {
  return {
    query: '',
    contentFilter: 'all',
    hideInstalled: false,
    installedUIDs: new Set(),
    versionFilter: '',
    categoryFilter: [],
    sortKey: 'downloads',
    sortDirection: 'desc',
    ...overrides
  };
}
