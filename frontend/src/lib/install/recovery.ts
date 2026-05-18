import type { TaskProgress } from '$lib/stores/downloads.svelte';

export type InstallFailureStage =
  | 'download'
  | 'integrity'
  | 'preflight'
  | 'extraction'
  | 'commit'
  | 'cancelled'
  | 'unknown';

export type InstallRecoveryGuidance = {
  stage: InstallFailureStage;
  title: string;
  action: string;
  diagnostics: string;
};

export function getInstallRecoveryGuidance(task: TaskProgress): InstallRecoveryGuidance | null {
  if (task.state !== 'failed' && task.state !== 'cancelled') return null;

  const error = task.error?.trim() ?? '';
  const lower = error.toLowerCase();

  if (task.state === 'cancelled') {
    return guidance(task, 'cancelled', 'Install cancelled', 'Retry when you are ready. No manual AddOns cleanup is required.', error);
  }

  if (lower.includes('md5 mismatch') || lower.includes('compute md5')) {
    return guidance(
      task,
      'integrity',
      'Download integrity check failed',
      'Retry once. If it repeats, refresh the ESOUI catalog and try again later.',
      error
    );
  }

  if (lower.includes('download ') || lower.includes('status') || lower.includes('network')) {
    return guidance(
      task,
      'download',
      'Download failed',
      'Check your connection, refresh the ESOUI catalog, then retry this task.',
      error
    );
  }

  if (
    lower.includes('preflight') ||
    lower.includes('open zip') ||
    lower.includes('archive') ||
    lower.includes('canonical manifest') ||
    lower.includes('escapes destination') ||
    lower.includes('invalid addon folder') ||
    lower.includes('not listed by esoui metadata')
  ) {
    return guidance(
      task,
      'preflight',
      'Archive preflight blocked install',
      'Do not manually merge the archive. Refresh the catalog or report the ESOUI package metadata.',
      error
    );
  }

  if (lower.includes('extract') || lower.includes('write file') || lower.includes('mkdir')) {
    return guidance(
      task,
      'extraction',
      'Extraction failed',
      'Retry after closing ESO and checking AddOns folder permissions or disk space.',
      error
    );
  }

  if (
    lower.includes('backup existing addon folder') ||
    lower.includes('install addon folder') ||
    lower.includes('rollback') ||
    lower.includes('staged addon folder')
  ) {
    return guidance(
      task,
      'commit',
      'Commit failed during AddOns update',
      'Retry after closing ESO and checking folder permissions. Scribe attempts rollback automatically.',
      error
    );
  }

  return guidance(
    task,
    'unknown',
    'Install failed',
    'Retry once. If it repeats, export diagnostics from Settings and include this task error.',
    error
  );
}

function guidance(
  task: TaskProgress,
  stage: InstallFailureStage,
  title: string,
  action: string,
  error: string
): InstallRecoveryGuidance {
  return {
    stage,
    title,
    action,
    diagnostics: [`uid=${task.uid}`, `name=${task.name || task.uid}`, `stage=${stage}`, `error=${error || 'n/a'}`].join('\n')
  };
}
