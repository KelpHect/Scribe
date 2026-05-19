import { describe, expect, it } from 'vitest';
import type { TaskProgress } from '$lib/stores/downloads.svelte';
import { getInstallRecoveryGuidance } from './recovery';

function task(error: string, state: TaskProgress['state'] = 'failed'): TaskProgress {
  return {
    uid: '101',
    name: 'LibFoo',
    state,
    percent: 0,
    bytesDownloaded: 0,
    totalBytes: 0,
    speed: 0,
    filesExtracted: 0,
    totalFiles: 0,
    queuePosition: 0,
    error
  };
}

describe('getInstallRecoveryGuidance', () => {
  it('classifies download and integrity failures', () => {
    expect(getInstallRecoveryGuidance(task('download https://example returned 500'))).toMatchObject(
      {
        stage: 'download',
        title: 'Download failed'
      }
    );
    expect(getInstallRecoveryGuidance(task('MD5 mismatch: expected a, got b'))).toMatchObject({
      stage: 'integrity',
      title: 'Download integrity check failed'
    });
  });

  it('classifies preflight and commit failures without unsafe delete advice', () => {
    const preflight = getInstallRecoveryGuidance(
      task('archive contains invalid addon folder name: ../Bad')
    );
    const commit = getInstallRecoveryGuidance(
      task('backup existing addon folder SkyShards: permission denied')
    );

    expect(preflight).toMatchObject({ stage: 'preflight' });
    expect(commit).toMatchObject({ stage: 'commit' });
    expect(`${preflight?.action} ${commit?.action}`.toLowerCase()).not.toContain('delete addons');
  });

  it('returns copyable diagnostics for failed and cancelled tasks only', () => {
    expect(getInstallRecoveryGuidance(task('', 'complete'))).toBeNull();
    const cancelled = getInstallRecoveryGuidance(task('', 'cancelled'));
    expect(cancelled).toMatchObject({ stage: 'cancelled' });
    expect(cancelled?.diagnostics).toContain('uid=101');
    expect(cancelled?.diagnostics).toContain('stage=cancelled');
  });
});
