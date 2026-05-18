export type AddonDecisionTone = 'neutral' | 'success' | 'warning' | 'destructive';

export interface AddonDecisionInput {
  installed: boolean;
  updateAvailable?: boolean;
  updateState?: string;
  updateReason?: string;
  localVersion?: string;
  remoteVersion?: string;
  folderName?: string;
}

export interface AddonDecision {
  label: string;
  tone: AddonDecisionTone;
  reason: string;
  localVersion: string;
  remoteVersion: string;
  folderName: string;
}

function clean(value: string | null | undefined): string {
  return value?.trim() ?? '';
}

export function describeAddonDecision(input: AddonDecisionInput): AddonDecision {
  const localVersion = clean(input.localVersion);
  const remoteVersion = clean(input.remoteVersion);
  const folderName = clean(input.folderName);
  const updateState = clean(input.updateState);
  const updateReason = clean(input.updateReason);

  if (!input.installed) {
    return {
      label: 'Ready to install',
      tone: 'neutral',
      reason: remoteVersion
        ? `ESOUI will install version ${remoteVersion}.`
        : 'ESOUI has an installable download for this addon.',
      localVersion,
      remoteVersion,
      folderName
    };
  }

  if (input.updateAvailable) {
    return {
      label: 'Update available',
      tone: 'destructive',
      reason:
        updateReason ||
        (remoteVersion
          ? `The ESOUI version ${remoteVersion} is newer than the installed copy.`
          : 'ESOUI reports a newer installable copy.'),
      localVersion,
      remoteVersion,
      folderName
    };
  }

  if (updateState === 'local-newer') {
    return {
      label: 'Local version is newer',
      tone: 'warning',
      reason:
        updateReason ||
        'The installed addon version sorts newer than ESOUI, so Scribe will not offer an update.',
      localVersion,
      remoteVersion,
      folderName
    };
  }

  if (updateState === 'md5-only-changed') {
    return {
      label: 'Same version changed upstream',
      tone: 'warning',
      reason:
        updateReason ||
        'ESOUI has a changed download for the same version, so updating can replace local files with the latest package.',
      localVersion,
      remoteVersion,
      folderName
    };
  }

  if (updateState === 'unknown-version') {
    return {
      label: 'Version cannot be compared',
      tone: 'warning',
      reason:
        updateReason ||
        'Scribe could not compare local and ESOUI versions; use the visible versions before deciding.',
      localVersion,
      remoteVersion,
      folderName
    };
  }

  if (updateState === 'unmatched') {
    return {
      label: 'No ESOUI match',
      tone: 'warning',
      reason: updateReason || 'This local addon is not matched to an ESOUI catalog entry.',
      localVersion,
      remoteVersion,
      folderName
    };
  }

  return {
    label: 'Up to date',
    tone: 'success',
    reason: updateReason || 'The installed addon matches the latest known ESOUI version.',
    localVersion,
    remoteVersion,
    folderName
  };
}
