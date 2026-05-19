import { describe, expect, it } from 'vitest';
import {
  buildInstalledAddonIndex,
  buildRemoteDependencyUIDMap,
  filterInstalledAddons,
  flattenInstalledGroups,
  groupInstalledAddons,
  INSTALLED_LIBRARIES_ID,
  INSTALLED_UNCATEGORIZED_ID
} from './installed-addons-index';
import type { Addon } from '$lib/services/addon-service';
import type { Category, MatchedAddon, RemoteAddon } from '$lib/services/esoui-service';

const categories: Category[] = [
  {
    id: 'combat',
    name: 'Action Bar Mods',
    iconUrl: 'combat.png',
    parentId: '',
    parentIds: [],
    count: 0
  },
  {
    id: 'map',
    name: 'Map, Coords, Compasses',
    iconUrl: 'map.png',
    parentId: '',
    parentIds: [],
    count: 0
  },
  { id: 'lib', name: 'Libraries', iconUrl: 'library.png', parentId: '', parentIds: [], count: 0 }
];

function addon(input: Partial<Addon> & Pick<Addon, 'folderName' | 'title'>): Addon {
  const { folderName, title, ...overrides } = input;
  return {
    id: folderName,
    folderName,
    title,
    version: '1.0',
    author: 'Author',
    description: '',
    dependsOn: [],
    optionalDependsOn: [],
    savedVariables: [],
    apiVersion: '101046',
    addOnVersion: '1',
    isLibrary: false,
    enabled: true,
    path: `/tmp/AddOns/${folderName}`,
    ...overrides
  };
}

function remote(input: Partial<RemoteAddon> & Pick<RemoteAddon, 'uid' | 'uiName'>): RemoteAddon {
  const { uid, uiName, ...overrides } = input;
  return {
    uid,
    categoryId: 'combat',
    uiName,
    uiAuthorName: 'Remote Author',
    uiDate: '',
    uiVersion: '1.1',
    uiDirs: [],
    uiFileInfoUrl: '',
    uiDownloadTotal: 0,
    uiFavoriteTotal: 0,
    uiIMGThumbs: [],
    uiIMGs: [],
    compatabilities: [],
    siblings: [],
    ...overrides
  };
}

function match(input: Partial<MatchedAddon> & Pick<MatchedAddon, 'folderName'>): MatchedAddon {
  const { folderName, ...overrides } = input;
  return {
    folderName,
    details: null,
    localVersion: '1.0',
    remoteVersion: '1.1',
    updateAvailable: false,
    updateState: 'up-to-date',
    updateReason: '',
    remote: null,
    ...overrides
  };
}

describe('installed addon index', () => {
  it('precomputes search fields, match maps, category images, update actions, and counts', () => {
    const indexed = buildInstalledAddonIndex(
      [
        addon({ folderName: 'BanditsUserInterface', title: 'Bandits User Interface' }),
        addon({ folderName: 'LibAddonMenu-2.0', title: 'LibAddonMenu', isLibrary: true })
      ],
      [
        match({
          folderName: 'BanditsUserInterface',
          updateAvailable: true,
          updateState: 'remote-newer',
          updateReason: 'Remote version is newer.',
          remote: remote({
            uid: 'bandits',
            uiName: 'Bandits User Interface',
            categoryId: 'combat',
            uiIMGs: ['bandits.png']
          })
        }),
        match({
          folderName: 'LibAddonMenu-2.0',
          remote: remote({ uid: 'lib', uiName: 'LibAddonMenu', categoryId: 'lib' })
        })
      ],
      categories
    );

    expect(indexed.totalCount).toBe(2);
    expect(indexed.libraryCount).toBe(1);
    expect(indexed.updatesAvailable).toHaveLength(1);
    expect(indexed.installedFolderNames.has('banditsuserinterface')).toBe(true);
    expect(indexed.matchedMap.get('banditsuserinterface')?.remote?.uid).toBe('bandits');
    expect(indexed.addons[0]).toMatchObject({
      titleLower: 'bandits user interface',
      folderLower: 'banditsuserinterface',
      listIconUrl: 'bandits.png',
      listIconIsThumbnail: true,
      hasUpdate: true
    });
    expect(indexed.addons[0].updateAction.label).toBe('Update to ESOUI version');
    expect(indexed.libraryCategoryIconUrl).toBe('library.png');
  });

  it('filters by precomputed title, author, and folder fields', () => {
    const indexed = buildInstalledAddonIndex(
      [
        addon({ folderName: 'MapPins', title: 'Map Pins', author: 'Map Author' }),
        addon({ folderName: 'SkyShards', title: 'SkyShards', author: 'Shards Team' })
      ],
      [],
      categories
    );

    expect(
      filterInstalledAddons(indexed.addons, 'map').map((item) => item.addon.folderName)
    ).toEqual(['MapPins']);
    expect(
      filterInstalledAddons(indexed.addons, 'shards team').map((item) => item.addon.folderName)
    ).toEqual(['SkyShards']);
  });

  it('groups non-library addons by remote category and keeps libraries last', () => {
    const indexed = buildInstalledAddonIndex(
      [
        addon({ folderName: 'NoMatch', title: 'No Match' }),
        addon({ folderName: 'LibFoo', title: 'Lib Foo', isLibrary: true }),
        addon({ folderName: 'MapPins', title: 'Map Pins' })
      ],
      [
        match({
          folderName: 'MapPins',
          remote: remote({ uid: 'map-pins', uiName: 'Map Pins', categoryId: 'map' })
        }),
        match({
          folderName: 'LibFoo',
          remote: remote({ uid: 'lib-foo', uiName: 'Lib Foo', categoryId: 'lib' })
        })
      ],
      categories
    );
    const groups = groupInstalledAddons(indexed.addons, indexed.libraryCategoryIconUrl);

    expect(groups.map((group) => group.id)).toEqual([
      'map',
      INSTALLED_UNCATEGORIZED_ID,
      INSTALLED_LIBRARIES_ID
    ]);
    expect(groups[0].addons.map((item) => item.addon.folderName)).toEqual(['MapPins']);
    expect(groups[2].iconUrl).toBe('library.png');

    expect(flattenInstalledGroups(groups, new Set(['map', INSTALLED_LIBRARIES_ID]))).toEqual([
      { type: 'header', group: groups[0] },
      { type: 'addon', item: groups[0].addons[0], groupId: 'map' },
      { type: 'header', group: groups[1] },
      { type: 'header', group: groups[2] },
      { type: 'addon', item: groups[2].addons[0], groupId: INSTALLED_LIBRARIES_ID }
    ]);
  });

  it('builds dependency lookup keys from remote dirs and names without overwriting earlier canonical matches', () => {
    const map = buildRemoteDependencyUIDMap([
      remote({ uid: 'first', uiName: 'LibFoo', uiDirs: ['LibFoo'] }),
      remote({ uid: 'second', uiName: 'Lib Foo', uiDirs: ['LibFoo', 'LibFoo-Alt'] })
    ]);

    expect(map.get('libfoo')).toBe('first');
    expect(map.get('lib foo')).toBe('second');
    expect(map.get('libfoo-alt')).toBe('second');
  });
});
