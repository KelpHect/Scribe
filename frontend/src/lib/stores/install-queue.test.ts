import { describe, expect, it } from 'vitest';
import { filterNewInstallUIDs } from './install-queue';

describe('filterNewInstallUIDs', () => {
  it('dedupes trimmed install UIDs and drops empty values', () => {
    const got = filterNewInstallUIDs([' 101 ', '', '101', '202', '  '], () => false);

    expect(got).toEqual(['101', '202']);
  });

  it('skips UIDs that are already pending or active', () => {
    const active = new Set(['101', '303']);
    const got = filterNewInstallUIDs(['101', '202', '303', '202', '404'], (uid) =>
      active.has(uid)
    );

    expect(got).toEqual(['202', '404']);
  });
});
