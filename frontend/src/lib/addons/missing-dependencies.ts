import type { MissingDepInfo } from '$lib/services/esoui-service';

export type MissingDependencyRow = {
  dep: MissingDepInfo;
  name: string;
  statusLabel: 'Installable' | 'Unresolved';
  requiredLabel: 'Required' | 'Optional';
  requiredByLabel: string;
  planText: string;
};

export type MissingDependencyDisplayPlan = {
  requiredCount: number;
  optionalCount: number;
  installable: MissingDepInfo[];
  unresolved: MissingDepInfo[];
  installablePreview: string;
  unresolvedPreview: string;
  rows: MissingDependencyRow[];
  hiddenCount: number;
};

const latestCanonicalInstallText =
  'Scribe will install the latest canonical ESOUI addon page for this dependency.';

export function buildMissingDependencyDisplayPlan(
  deps: readonly MissingDepInfo[],
  rowLimit = 5
): MissingDependencyDisplayPlan {
  const installable = deps.filter((dep) => dep.canInstall);
  const unresolved = deps.filter((dep) => !dep.canInstall);

  return {
    requiredCount: deps.filter((dep) => !dep.optional).length,
    optionalCount: deps.filter((dep) => dep.optional).length,
    installable,
    unresolved,
    installablePreview: preview(installable.map(dependencyName), 4),
    unresolvedPreview: preview(
      unresolved.map((dep) => dep.depFolderName),
      3
    ),
    rows: deps.slice(0, rowLimit).map(toRow),
    hiddenCount: Math.max(0, deps.length - rowLimit)
  };
}

export function dependencyName(dep: MissingDepInfo): string {
  return dep.remoteName || dep.depFolderName;
}

function toRow(dep: MissingDepInfo): MissingDependencyRow {
  return {
    dep,
    name: dependencyName(dep),
    statusLabel: dep.canInstall ? 'Installable' : 'Unresolved',
    requiredLabel: dep.optional ? 'Optional' : 'Required',
    requiredByLabel: formatRequiredBy(dep.requiredBy),
    planText: dep.canInstall ? latestCanonicalInstallText : dep.planReason
  };
}

function formatRequiredBy(requiredBy: readonly string[]): string {
  if (requiredBy.length === 0) return '';
  return `Used by ${requiredBy.slice(0, 2).join(', ')}${
    requiredBy.length > 2 ? ` +${requiredBy.length - 2} more` : ''
  }`;
}

function preview(values: readonly string[], limit: number): string {
  const visible = values.slice(0, limit).join(', ');
  const hidden = values.length - limit;
  return hidden > 0 ? `${visible} +${hidden} more` : visible;
}
