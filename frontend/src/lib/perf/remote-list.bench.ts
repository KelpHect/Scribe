import { bench, describe } from 'vitest';
import type { RemoteAddon } from '$lib/services/esoui-service';
import {
  getLatestCompatibility,
  isLibraryLikeRemoteAddon,
  remoteAddonSearchScore
} from './remote-list';

function makeRemoteAddons(count: number): RemoteAddon[] {
  return Array.from({ length: count }, (_, i) => ({
    uid: String(i),
    categoryId: i % 7 === 0 ? 'libraries' : 'addons',
    uiName: i % 7 === 0 ? `Lib Bench ${i}` : `Bench Addon ${i}`,
    uiAuthorName: `Author ${i % 40}`,
    uiDate: '2026-05-18',
    uiVersion: `1.${i % 10}`,
    uiDirs: [`BenchAddon${i}`, i % 7 === 0 ? `LibBench${i}` : `BenchExtra${i}`],
    uiFileInfoUrl: 'https://example.invalid/addon',
    uiDownloadTotal: i * 10,
    uiFavoriteTotal: i,
    uiIMGThumbs: [],
    uiIMGs: [],
    compatabilities: [
      { name: 'ESO', version: '9.3.0' },
      { name: 'ESO', version: '10.0.0' }
    ],
    siblings: []
  }));
}

describe('remote list preparation benchmarks', () => {
  const addons = makeRemoteAddons(7000);

  bench('remote search score over large cached catalog', () => {
    addons
      .map((addon) => ({ addon, score: remoteAddonSearchScore(addon, 'bench addon 42') }))
      .filter((entry) => entry.score > 0)
      .sort((a, b) => b.score - a.score);
  });

  bench('remote filter metadata preparation', () => {
    addons.map((addon) => ({
      latestCompatibility: getLatestCompatibility(addon.compatabilities),
      libraryLike: isLibraryLikeRemoteAddon(addon, addon.categoryId)
    }));
  });
});
