import { describe, expect, it } from 'vitest';
import { describeAddonDecision } from './decision';

describe('describeAddonDecision', () => {
  it('describes a new install with the remote version', () => {
    expect(describeAddonDecision({ installed: false, remoteVersion: '1.2.3' })).toMatchObject({
      label: 'Ready to install',
      tone: 'neutral',
      reason: 'ESOUI will install version 1.2.3.'
    });
  });

  it('prefers update reasons for available updates', () => {
    expect(
      describeAddonDecision({
        installed: true,
        updateAvailable: true,
        updateReason: 'Remote version sorts newer.',
        localVersion: '1.0.0',
        remoteVersion: '1.1.0'
      })
    ).toMatchObject({
      label: 'Update available',
      tone: 'destructive',
      reason: 'Remote version sorts newer.',
      localVersion: '1.0.0',
      remoteVersion: '1.1.0'
    });
  });

  it('warns when the local version is newer than the catalog', () => {
    expect(
      describeAddonDecision({
        installed: true,
        updateState: 'local-newer',
        localVersion: '2.0.0',
        remoteVersion: '1.9.0'
      })
    ).toMatchObject({
      label: 'Local version is newer',
      tone: 'warning'
    });
  });

  it('explains same-version remote package changes', () => {
    expect(
      describeAddonDecision({
        installed: true,
        updateState: 'md5-only-changed',
        localVersion: '1.0.0',
        remoteVersion: '1.0.0'
      })
    ).toMatchObject({
      label: 'Same version changed upstream',
      tone: 'warning'
    });
  });

  it('defaults matched installed addons to up to date', () => {
    expect(describeAddonDecision({ installed: true, updateState: 'up-to-date' })).toMatchObject({
      label: 'Up to date',
      tone: 'success'
    });
  });
});
