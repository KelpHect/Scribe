import { describe, expect, it } from 'vitest';
import type { MissingDepInfo } from '$lib/services/esoui-service';
import { buildMissingDependencyDisplayPlan, dependencyName } from './missing-dependencies';

function dep(input: Partial<MissingDepInfo> = {}): MissingDepInfo {
  return {
    depFolderName: 'LibFoo',
    requiredBy: ['AddonOne'],
    versionConstraints: [],
    remoteUID: '',
    remoteName: '',
    canInstall: true,
    optional: false,
    planState: 'installable',
    planReason: 'Matched the latest canonical ESOUI addon entry.',
    ...input
  };
}

describe('missing dependency display planning', () => {
  it('prefers remote names for installable display names', () => {
    expect(dependencyName(dep({ depFolderName: 'LibGPS', remoteName: 'Lib GPS' }))).toBe('Lib GPS');
    expect(dependencyName(dep({ depFolderName: 'LibGPS', remoteName: '' }))).toBe('LibGPS');
  });

  it('separates counts and previews required, optional, installable, and unresolved dependencies', () => {
    const plan = buildMissingDependencyDisplayPlan([
      dep({ depFolderName: 'LibA', remoteName: 'Library A', optional: false, canInstall: true }),
      dep({
        depFolderName: 'LibB',
        optional: true,
        canInstall: false,
        planReason: 'No catalog match.'
      }),
      dep({
        depFolderName: 'LibC',
        optional: false,
        canInstall: false,
        planReason: 'No catalog match.'
      })
    ]);

    expect(plan.requiredCount).toBe(2);
    expect(plan.optionalCount).toBe(1);
    expect(plan.installablePreview).toBe('Library A');
    expect(plan.unresolvedPreview).toBe('LibB, LibC');
  });

  it('builds bounded rows with required-by labels and install plan text', () => {
    const plan = buildMissingDependencyDisplayPlan(
      [
        dep({ depFolderName: 'LibA', requiredBy: ['One', 'Two', 'Three'] }),
        dep({
          depFolderName: 'LibB',
          canInstall: false,
          optional: true,
          planReason: 'No ESOUI match.'
        })
      ],
      1
    );

    expect(plan.rows).toHaveLength(1);
    expect(plan.rows[0]).toMatchObject({
      name: 'LibA',
      statusLabel: 'Installable',
      requiredLabel: 'Required',
      requiredByLabel: 'Used by One, Two +1 more'
    });
    expect(plan.rows[0].planText).toContain('latest canonical ESOUI addon page');
    expect(plan.hiddenCount).toBe(1);
  });
});
