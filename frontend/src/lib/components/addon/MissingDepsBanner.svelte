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
</script>

<div
  class="card-elevated border-warning/40 bg-warning/10 flex items-start gap-3 rounded-lg border p-3"
>
  <AlertTriangle size={16} class="text-warning mt-0.5 shrink-0" />
  <div class="min-w-0 flex-1">
    <p class="text-foreground text-sm font-semibold">{title}</p>
    <div class="text-muted-foreground mt-1 space-y-1 text-xs">
      {#if installableDeps.length > 0}
        <p>
          Installable:
          {installableDeps
            .map((d) => d.remoteName || d.depFolderName)
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
      {#each deps.slice(0, 3) as dep (dep.depFolderName)}
        <p class="truncate">
          <span class="text-foreground font-mono">{dep.depFolderName}</span>
          {dep.optional ? 'optional' : 'required'} · {dep.planReason}
          {#if dep.versionConstraints.length > 0}
            · {dep.versionConstraints.join(', ')}
          {/if}
        </p>
      {/each}
    </div>
  </div>
  <div class="flex shrink-0 items-center gap-2">
    <button
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
      onclick={ondismiss}
      class="text-muted-foreground hover:text-foreground text-xs transition-colors"
      aria-label="Dismiss"
    >
      Dismiss
    </button>
  </div>
</div>
