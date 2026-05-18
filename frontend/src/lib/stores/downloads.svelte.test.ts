import { describe, expect, it } from 'vitest';
import {
  shouldApplyDownloadProgressImmediately,
  type TaskProgress
} from './downloads.svelte';

function task(input: Partial<TaskProgress> = {}): TaskProgress {
  return {
    uid: '101',
    name: 'Addon',
    state: 'downloading',
    percent: 10,
    bytesDownloaded: 10,
    totalBytes: 100,
    speed: 1,
    filesExtracted: 0,
    totalFiles: 0,
    queuePosition: 0,
    ...input
  };
}

describe('download progress coalescing', () => {
  it('applies first events, state transitions, and terminal states immediately', () => {
    expect(shouldApplyDownloadProgressImmediately(task(), undefined)).toBe(true);
    expect(
      shouldApplyDownloadProgressImmediately(task({ state: 'extracting' }), task({ state: 'downloading' }))
    ).toBe(true);
    expect(
      shouldApplyDownloadProgressImmediately(task({ state: 'complete' }), task({ state: 'complete' }))
    ).toBe(true);
    expect(
      shouldApplyDownloadProgressImmediately(task({ state: 'failed' }), task({ state: 'failed' }))
    ).toBe(true);
    expect(
      shouldApplyDownloadProgressImmediately(task({ state: 'cancelled' }), task({ state: 'cancelled' }))
    ).toBe(true);
  });

  it('batches byte-only progress updates within the same active state', () => {
    expect(
      shouldApplyDownloadProgressImmediately(
        task({ percent: 40, bytesDownloaded: 40 }),
        task({ percent: 10, bytesDownloaded: 10 })
      )
    ).toBe(false);
    expect(
      shouldApplyDownloadProgressImmediately(
        task({ state: 'extracting', filesExtracted: 10 }),
        task({ state: 'extracting', filesExtracted: 1 })
      )
    ).toBe(false);
  });
});
