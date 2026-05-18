import type { InstallPlanEntry } from '$lib/stores/downloads.svelte';

export type InstallPlanCounts = {
  add: number;
  replace: number;
  total: number;
};

export function getInstallPlanCounts(plan: readonly InstallPlanEntry[] = []): InstallPlanCounts {
  let add = 0;
  let replace = 0;

  for (const item of plan) {
    if (item.action === 'replace') {
      replace++;
    } else {
      add++;
    }
  }

  return { add, replace, total: plan.length };
}

export function formatInstallPlanSummary(plan: readonly InstallPlanEntry[] = []): string {
  const counts = getInstallPlanCounts(plan);
  if (counts.total === 0) return 'No folder changes planned yet';

  const parts: string[] = [];
  if (counts.add > 0) parts.push(`${counts.add} add`);
  if (counts.replace > 0) parts.push(`${counts.replace} replace`);
  return parts.join(' · ');
}

export function getInstallPlanSafetyNote(plan: readonly InstallPlanEntry[] = []): string {
  const counts = getInstallPlanCounts(plan);
  if (counts.total === 0) return 'Scribe validates the archive before touching AddOns.';
  if (counts.replace > 0) {
    return 'Existing folders are backed up during commit and restored if the install fails.';
  }
  return 'New folders are staged first and copied into AddOns only after preflight passes.';
}
