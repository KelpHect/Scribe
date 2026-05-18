import { describe, expect, it } from 'vitest';
import { getLatestCompatibility } from './remote-list';

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
