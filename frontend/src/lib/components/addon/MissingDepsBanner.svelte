<svelte:options runes />

<script lang="ts">
  import AlertTriangle from 'lucide-svelte/icons/alert-triangle';
  import Download from 'lucide-svelte/icons/download';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import type { MissingDepInfo } from '$lib/services/esoui-service';
  import { buildMissingDependencyDisplayPlan } from '$lib/addons/missing-dependencies';

  interface Props {
    deps: MissingDepInfo[];
    title: string;
    actionLabel: string;
    batchInstalling: boolean;
    oninstall: () => void;
    ondismiss: () => void;
  }

  const { deps, title, actionLabel, batchInstalling, oninstall, ondismiss }: Props = $props();

  const display = $derived(buildMissingDependencyDisplayPlan(deps));
</script>

<div
  class="card-elevated border-warning/40 bg-warning/10 flex items-start gap-3 rounded-lg border p-3"
>
  <AlertTriangle size={16} class="text-warning mt-0.5 shrink-0" />
  <div class="min-w-0 flex-1">
    <p class="text-foreground text-sm font-semibold">{title}</p>
    <div class="text-muted-foreground mt-1 space-y-1 text-xs">
      <p>
        {display.requiredCount} required · {display.optionalCount} optional · {display.installable.length} installable · {display.unresolved.length} unresolved
      </p>
      {#if display.installable.length > 0}
        <p>
          Installable:
          {display.installablePreview}
        </p>
      {/if}
      {#if display.unresolved.length > 0}
        <p>
          Unresolved:
          {display.unresolvedPreview}
        </p>
      {/if}
      <div class="mt-2 grid gap-1.5">
        {#each display.rows as row (row.dep.depFolderName)}
          <div class="rounded-md border border-[var(--color-border)] bg-background/60 px-2 py-1.5">
            <div class="flex items-center justify-between gap-2">
              <span class="min-w-0 truncate text-foreground font-mono">{row.dep.depFolderName}</span>
              <span
                class={row.dep.canInstall
                  ? 'shrink-0 rounded border border-success/40 bg-success/10 px-1.5 py-0.5 text-[10px] text-success'
                  : 'shrink-0 rounded border px-1.5 py-0.5 text-[10px]'}
              >
                {row.statusLabel}
              </span>
            </div>
            <p class="mt-1 line-clamp-2">
              <span class="font-medium">{row.requiredLabel}</span>
              {#if row.dep.remoteName}
                · latest ESOUI match: {row.dep.remoteName}
              {/if}
              {#if row.dep.versionConstraints.length > 0}
                · requested {row.dep.versionConstraints.join(', ')}
              {/if}
              {#if row.requiredByLabel}
                · {row.requiredByLabel}
              {/if}
            </p>
            <p class="mt-0.5 line-clamp-2">
              {row.planText}
            </p>
          </div>
        {/each}
        {#if display.hiddenCount > 0}
          <p>+{display.hiddenCount} more dependencies not shown</p>
        {/if}
      </div>
    </div>
  </div>
  <div class="flex shrink-0 items-center gap-2">
    <button
      type="button"
      onclick={oninstall}
      disabled={batchInstalling || display.installable.length === 0}
      class="bg-primary text-primary-foreground hover:bg-primary/90 inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50"
    >
      {#if batchInstalling}
        <Loader2 size={11} class="animate-spin" />Installing...
      {:else}
        <Download size={11} />{actionLabel} ({display.installable.length})
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
