import { describe, expect, it } from 'vitest';
import type { TaskProgress } from '$lib/stores/downloads.svelte';
import { summarizeTaskProgress } from './task-summary';

function task(input: Partial<TaskProgress> = {}): TaskProgress {
  return {
    uid: '123',
    name: 'SkyShards',
    state: 'downloading',
    percent: 42,
    bytesDownloaded: 512,
    totalBytes: 1024,
    speed: 256,
    filesExtracted: 0,
    totalFiles: 0,
    queuePosition: 0,
    ...input
  };
}

describe('task display summaries', () => {
  it('formats active download labels', () => {
    expect(summarizeTaskProgress(task())).toMatchObject({
      stateLabel: 'Downloading',
      speedLabel: '256 B/s',
      sizeLabel: '512 B / 1 KB',
      isActive: true,
      isTerminal: false,
      progressPercent: 42,
      expectedSizeLabel: '1 KB'
    });
  });

  it('formats extraction progress separately from download size', () => {
    expect(
      summarizeTaskProgress(task({ state: 'extracting', filesExtracted: 3, totalFiles: 10 }))
    ).toMatchObject({
      stateLabel: 'Extracting',
      sizeLabel: '',
      extractionLabel: '3/10 files',
      isActive: true
    });
  });

  it('clamps percent and marks terminal states', () => {
    expect(summarizeTaskProgress(task({ state: 'failed', percent: 125 }))).toMatchObject({
      stateLabel: 'Failed',
      progressPercent: 100,
      isActive: false,
      isTerminal: true
    });
  });
});
