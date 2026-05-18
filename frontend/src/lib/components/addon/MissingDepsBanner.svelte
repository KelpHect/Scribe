<svelte:options runes />

<script lang="ts">
  import AlertTriangle from 'lucide-svelte/icons/alert-triangle';
  import Download from 'lucide-svelte/icons/download';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import type { MissingDepInfo } from '$lib/services/esoui-service';

  interface Props {
    deps: MissingDepInfo[];
    title: string;
    actionLabel: string;
    batchInstalling: boolean;
    oninstall: () => void;
    ondismiss: () => void;
  }

  const { deps, title, actionLabel, batchInstalling, oninstall, ondismiss }: Props = $props();

  const installableDeps = $derived(deps.filter((d) => d.canInstall));
  const unresolvedDeps = $derived(deps.filter((d) => !d.canInstall));
  const requiredCount = $derived(deps.filter((d) => !d.optional).length);
  const optionalCount = $derived(deps.filter((d) => d.optional).length);

  function depName(dep: MissingDepInfo): string {
    return dep.remoteName || dep.depFolderName;
  }

  function requiredByLabel(dep: MissingDepInfo): string {
    if (dep.requiredBy.length === 0) return '';
    return `Used by ${dep.requiredBy.slice(0, 2).join(', ')}${
      dep.requiredBy.length > 2 ? ` +${dep.requiredBy.length - 2} more` : ''
    }`;
  }
</script>

<div
  class="card-elevated border-warning/40 bg-warning/10 flex items-start gap-3 rounded-lg border p-3"
>
  <AlertTriangle size={16} class="text-warning mt-0.5 shrink-0" />
  <div class="min-w-0 flex-1">
    <p class="text-foreground text-sm font-semibold">{title}</p>
    <div class="text-muted-foreground mt-1 space-y-1 text-xs">
      <p>
        {requiredCount} required · {optionalCount} optional · {installableDeps.length} installable · {unresolvedDeps.length} unresolved
      </p>
      {#if installableDeps.length > 0}
        <p>
          Installable:
          {installableDeps
            .map((d) => depName(d))
            .slice(0, 4)
            .join(', ')}{installableDeps.length > 4 ? ` +${installableDeps.length - 4} more` : ''}
        </p>
      {/if}
      {#if unresolvedDeps.length > 0}
        <p>
          Unresolved:
          {unresolvedDeps
            .map((d) => d.depFolderName)
            .slice(0, 3)
            .join(', ')}{unresolvedDeps.length > 3 ? ` +${unresolvedDeps.length - 3} more` : ''}
        </p>
      {/if}
      <div class="mt-2 grid gap-1.5">
        {#each deps.slice(0, 5) as dep (dep.depFolderName)}
          <div class="rounded-md border border-[var(--color-border)] bg-background/60 px-2 py-1.5">
            <div class="flex items-center justify-between gap-2">
              <span class="min-w-0 truncate text-foreground font-mono">{dep.depFolderName}</span>
              <span
                class={dep.canInstall
                  ? 'shrink-0 rounded border border-success/40 bg-success/10 px-1.5 py-0.5 text-[10px] text-success'
                  : 'shrink-0 rounded border px-1.5 py-0.5 text-[10px]'}
              >
                {dep.canInstall ? 'Installable' : 'Unresolved'}
              </span>
            </div>
            <p class="mt-1 line-clamp-2">
              <span class="font-medium">{dep.optional ? 'Optional' : 'Required'}</span>
              {#if dep.remoteName}
                · latest ESOUI match: {dep.remoteName}
              {/if}
              {#if dep.versionConstraints.length > 0}
                · requested {dep.versionConstraints.join(', ')}
              {/if}
              {#if requiredByLabel(dep)}
                · {requiredByLabel(dep)}
              {/if}
            </p>
            <p class="mt-0.5 line-clamp-2">
              {dep.canInstall
                ? 'Scribe will install the latest canonical ESOUI addon page for this dependency.'
                : dep.planReason}
            </p>
          </div>
        {/each}
        {#if deps.length > 5}
          <p>+{deps.length - 5} more dependencies not shown</p>
        {/if}
      </div>
    </div>
  </div>
  <div class="flex shrink-0 items-center gap-2">
    <button
      type="button"
      onclick={oninstall}
      disabled={batchInstalling || installableDeps.length === 0}
      class="bg-primary text-primary-foreground hover:bg-primary/90 inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50"
    >
      {#if batchInstalling}
        <Loader2 size={11} class="animate-spin" />Installing...
      {:else}
        <Download size={11} />{actionLabel} ({installableDeps.length})
      {/if}
    </button>
    <button
      type="button"
      onclick={ondismiss}
      class="text-muted-foreground hover:text-foreground text-xs transition-colors"
      aria-label="Dismiss"
    >
      Dismiss
    </button>
  </div>
</div>
