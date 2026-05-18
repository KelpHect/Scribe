import { describe, expect, it } from 'vitest';
import type { Addon } from '$lib/services/addon-service';
import type { MatchedAddon, MissingDepInfo } from '$lib/services/esoui-service';
import { buildAddonHealthSummary } from './health';

function addon(overrides: Partial<Addon>): Addon {
  return {
    id: overrides.folderName ?? 'Addon',
    folderName: 'Addon',
    title: 'Addon',
    version: '1',
    author: 'Author',
    description: '',
    dependsOn: [],
    optionalDependsOn: [],
    savedVariables: [],
    apiVersion: '',
    addOnVersion: '',
    isLibrary: false,
    enabled: true,
    path: '',
    ...overrides
  };
}

function match(overrides: Partial<MatchedAddon>): MatchedAddon {
  return {
    folderName: 'Addon',
    remote: null,
    details: null,
    updateAvailable: false,
    localVersion: '',
    remoteVersion: '',
    updateState: 'up-to-date',
    updateReason: '',
    ...overrides
  };
}

function missingDep(overrides: Partial<MissingDepInfo>): MissingDepInfo {
  return {
    depFolderName: 'LibMissing',
    requiredBy: ['Addon'],
    versionConstraints: [],
    remoteUID: '',
    remoteName: '',
    canInstall: false,
    optional: false,
    planState: 'unresolved',
    planReason: '',
    ...overrides
  };
}

describe('buildAddonHealthSummary', () => {
  it('flags missing required dependencies separately from optional ones', () => {
    const summary = buildAddonHealthSummary(
      [addon({ folderName: 'Addon' })],
      [match({ folderName: 'Addon', remote: {} as MatchedAddon['remote'] })],
      [
        missingDep({ depFolderName: 'LibRequired', optional: false, canInstall: true }),
        missingDep({ depFolderName: 'LibOptional', optional: true })
      ]
    );

    expect(summary.issues.find((issue) => issue.key === 'missing-required')).toMatchObject({
      count: 1,
      actionLabel: 'Install required',
      dependencyFolderNames: ['LibRequired']
    });
  });

  it('flags updateable matched addons from metadata', () => {
    const summary = buildAddonHealthSummary(
      [addon({ folderName: 'NeedsUpdate' })],
      [match({ folderName: 'NeedsUpdate', updateAvailable: true })],
      []
    );

    expect(summary.issues.find((issue) => issue.key === 'outdated')).toMatchObject({
      count: 1,
      addonFolderNames: ['NeedsUpdate']
    });
  });

  it('flags unknown local folders when no catalog match exists', () => {
    const summary = buildAddonHealthSummary([addon({ folderName: 'ManualAddon' })], [], []);

    expect(summary.issues.find((issue) => issue.key === 'unknown')).toMatchObject({
      count: 1,
      addonFolderNames: ['ManualAddon']
    });
  });

  it('flags disabled and stub-like manifests where detectable', () => {
    const summary = buildAddonHealthSummary(
      [
        addon({ folderName: 'Disabled', title: 'Disabled', enabled: false }),
        addon({ folderName: 'StubOnly', title: 'StubOnly', version: '', author: '' })
      ],
      [],
      []
    );

    expect(summary.issues.find((issue) => issue.key === 'disabled-stub')).toMatchObject({
      count: 2,
      addonFolderNames: ['Disabled', 'StubOnly']
    });
  });
});
