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
</script>

<div
  class="card-elevated border-warning/40 bg-warning/10 flex items-start gap-3 rounded-lg border p-3"
>
  <AlertTriangle size={16} class="text-warning mt-0.5 shrink-0" />
  <div class="min-w-0 flex-1">
    <p class="text-foreground text-sm font-semibold">{title}</p>
    <p class="text-muted-foreground mt-0.5 text-xs">
      {deps
        .map((d) => d.remoteName || d.depFolderName)
        .slice(0, 4)
        .join(', ')}{deps.length > 4 ? ` +${deps.length - 4} more` : ''}
    </p>
  </div>
  <div class="flex shrink-0 items-center gap-2">
    <button
      onclick={oninstall}
      disabled={batchInstalling}
      class="bg-primary text-primary-foreground hover:bg-primary/90 inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50"
    >
      {#if batchInstalling}
        <Loader2 size={11} class="animate-spin" />Installing...
      {:else}
        <Download size={11} />{actionLabel}
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
