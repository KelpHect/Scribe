import { describe, expect, it } from 'vitest';
import { filterNewInstallUIDs, filterRetryInstallUIDs } from './install-queue';

describe('filterNewInstallUIDs', () => {
  it('dedupes trimmed install UIDs and drops empty values', () => {
    const got = filterNewInstallUIDs([' 101 ', '', '101', '202', '  '], () => false);

    expect(got).toEqual(['101', '202']);
  });

  it('skips UIDs that are already pending or active', () => {
    const active = new Set(['101', '303']);
    const got = filterNewInstallUIDs(['101', '202', '303', '202', '404'], (uid) => active.has(uid));

    expect(got).toEqual(['202', '404']);
  });
});

describe('filterRetryInstallUIDs', () => {
  it('only retries failed tasks and skips active duplicates', () => {
    const active = new Set(['303']);
    const got = filterRetryInstallUIDs(
      [
        { uid: '101', state: 'complete' },
        { uid: '202', state: 'failed' },
        { uid: '202', state: 'failed' },
        { uid: '303', state: 'failed' },
        { uid: '404', state: 'cancelled' }
      ],
      (uid) => active.has(uid)
    );

    expect(got).toEqual(['202']);
  });
});
