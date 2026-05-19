import type { Addon } from '$lib/services/addon-service';
import type { MatchedAddon, MissingDepInfo } from '$lib/services/esoui-service';

export interface AddonHealthIssue {
  key: string;
  title: string;
  severity: 'warning' | 'info';
  count: number;
  description: string;
  actionLabel?: string;
  addonFolderNames: string[];
  dependencyFolderNames: string[];
}

export interface AddonHealthSummary {
  issueCount: number;
  issues: AddonHealthIssue[];
}

function isStubLikeManifest(addon: Addon): boolean {
  return (
    addon.title.trim().toLowerCase() === addon.folderName.trim().toLowerCase() &&
    !addon.version &&
    !addon.author &&
    !addon.description &&
    (addon.dependsOn?.length ?? 0) === 0 &&
    (addon.optionalDependsOn?.length ?? 0) === 0 &&
    (addon.savedVariables?.length ?? 0) === 0
  );
}

export function buildAddonHealthSummary(
  addons: Addon[],
  matchedAddons: MatchedAddon[],
  missingDeps: MissingDepInfo[]
): AddonHealthSummary {
  const issues: AddonHealthIssue[] = [];
  const matchedByFolder = new Map(matchedAddons.map((m) => [m.folderName.toLowerCase(), m]));

  const missingRequired = missingDeps.filter((dep) => !dep.optional);
  if (missingRequired.length > 0) {
    issues.push({
      key: 'missing-required',
      title: 'Missing required libraries',
      severity: 'warning',
      count: missingRequired.length,
      description: 'Some installed addons declare required libraries that are not installed.',
      actionLabel: missingRequired.some((dep) => dep.canInstall) ? 'Install required' : undefined,
      addonFolderNames: Array.from(
        new Set(missingRequired.flatMap((dep) => dep.requiredBy))
      ).sort(),
      dependencyFolderNames: missingRequired.map((dep) => dep.depFolderName).sort()
    });
  }

  const outdated = matchedAddons
    .filter((match) => match.updateAvailable)
    .map((match) => match.folderName)
    .sort();
  if (outdated.length > 0) {
    issues.push({
      key: 'outdated',
      title: 'Updates available',
      severity: 'info',
      count: outdated.length,
      description: 'ESOUI metadata reports newer installable versions for these addons.',
      actionLabel: 'Queue updates',
      addonFolderNames: outdated,
      dependencyFolderNames: []
    });
  }

  const unknown = addons
    .filter((addon) => {
      const match = matchedByFolder.get(addon.folderName.toLowerCase());
      return !match || match.updateState === 'unmatched' || !match.remote;
    })
    .map((addon) => addon.folderName)
    .sort();
  if (unknown.length > 0) {
    issues.push({
      key: 'unknown',
      title: 'Unknown local folders',
      severity: 'info',
      count: unknown.length,
      description: 'These installed folders do not currently match an ESOUI catalog entry.',
      addonFolderNames: unknown,
      dependencyFolderNames: []
    });
  }

  const disabledOrStub = addons
    .filter((addon) => !addon.enabled || isStubLikeManifest(addon))
    .map((addon) => addon.folderName)
    .sort();
  if (disabledOrStub.length > 0) {
    issues.push({
      key: 'disabled-stub',
      title: 'Disabled or stub-like manifests',
      severity: 'warning',
      count: disabledOrStub.length,
      description: 'Scribe found disabled addons or manifests with almost no metadata.',
      addonFolderNames: disabledOrStub,
      dependencyFolderNames: []
    });
  }

  return {
    issueCount: issues.reduce((sum, issue) => sum + issue.count, 0),
    issues
  };
}
