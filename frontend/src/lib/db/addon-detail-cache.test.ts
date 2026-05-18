import { QueryClient } from '@tanstack/query-core';
import { describe, expect, it, vi } from 'vitest';
import type { RemoteAddonDetails } from '$lib/services/esoui-service';
import {
  ADDON_DETAIL_MAX_QUERIES,
  ADDON_DETAIL_MAX_SCREENSHOTS,
  addonDetailsQueryKey,
  getAddonDetailCacheStats,
  trimAddonDetailQueries
} from './addon-detail-cache';

function detail(uid: string, images: number): RemoteAddonDetails {
  return {
    uid,
    categoryId: 'cat',
    uiName: `Addon ${uid}`,
    uiAuthorName: 'Author',
    uiDate: '2026-05-18',
    uiVersion: '1.0',
    uiDirs: [`Addon${uid}`],
    uiFileInfoUrl: '',
    uiDownloadTotal: 0,
    uiFavoriteTotal: 0,
    uiIMGThumbs: Array.from({ length: images }, (_, i) => `thumb-${uid}-${i}`),
    uiIMGs: Array.from({ length: images }, (_, i) => `full-${uid}-${i}`),
    compatabilities: [],
    siblings: [],
    uiMD5: '',
    uiFileName: '',
    uiDownload: '',
    uiDescription: '',
    uiChangeLog: '',
    uiHitCount: 0,
    uiDonationLink: '',
    UIPending: false,
    uiCatId: 'cat'
  };
}

describe('addon detail cache bounds', () => {
  it('trims oldest detail queries while keeping the current detail query', () => {
    const queryClient = new QueryClient();

    vi.useFakeTimers();
    try {
      for (let i = 0; i < ADDON_DETAIL_MAX_QUERIES + 4; i++) {
        vi.setSystemTime(new Date(2026, 0, 1, 0, 0, i));
        queryClient.setQueryData(addonDetailsQueryKey(String(i)), detail(String(i), 0));
      }
    } finally {
      vi.useRealTimers();
    }

    trimAddonDetailQueries(queryClient, '0');

    const remaining = queryClient.getQueryCache().findAll({ queryKey: ['addon-details'] });
    expect(remaining).toHaveLength(ADDON_DETAIL_MAX_QUERIES);
    expect(queryClient.getQueryData(addonDetailsQueryKey('0'))).toBeTruthy();
    expect(queryClient.getQueryData(addonDetailsQueryKey('1'))).toBeUndefined();
  });

  it('reports bounded cache and screenshot stats for diagnostics', () => {
    const queryClient = new QueryClient();
    queryClient.setQueryData(addonDetailsQueryKey('101'), detail('101', ADDON_DETAIL_MAX_SCREENSHOTS));

    const stats = getAddonDetailCacheStats(queryClient);

    expect(stats).toMatchObject({
      totalQueries: 1,
      queriesWithData: 1,
      fresh: 1,
      stale: 0,
      cachedUIDs: ['101'],
      screenshotUrls: ADDON_DETAIL_MAX_SCREENSHOTS * 2,
      maxQueries: ADDON_DETAIL_MAX_QUERIES,
      maxScreenshotsPerDetail: ADDON_DETAIL_MAX_SCREENSHOTS
    });
  });
});
