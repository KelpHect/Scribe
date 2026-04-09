<svelte:options runes />

<script lang="ts">
  import AlertCircle from 'lucide-svelte/icons/alert-circle';
  import CheckCircle2 from 'lucide-svelte/icons/check-circle-2';
  import Clock from 'lucide-svelte/icons/clock';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import X from 'lucide-svelte/icons/x';
  import type { TaskProgress } from '$lib/stores/downloads.svelte';
  import { formatBytes } from '$lib/utils';

  type Props = {
    task: TaskProgress;
    oncancel?: (_uid: string) => void;
    ondismiss?: (_uid: string) => void;
  };

  const { task, oncancel, ondismiss }: Props = $props();

  const stateLabel = $derived.by(() => {
    switch (task.state) {
      case 'queued':
        return 'Queued';
      case 'downloading':
        return 'Downloading';
      case 'extracting':
        return 'Extracting';
      case 'complete':
        return 'Complete';
      case 'failed':
        return 'Failed';
      case 'cancelled':
        return 'Cancelled';
      default:
        return '';
    }
  });

  const speedLabel = $derived(
    task.state === 'downloading' && task.speed > 0 ? `${formatBytes(task.speed)}/s` : ''
  );

  const sizeLabel = $derived.by(() => {
    if (task.state !== 'downloading') return '';
    if (task.totalBytes > 0) {
      return `${formatBytes(task.bytesDownloaded)} / ${formatBytes(task.totalBytes)}`;
    }
    if (task.bytesDownloaded > 0) {
      return formatBytes(task.bytesDownloaded);
    }
    return '';
  });

  const isActive = $derived(
    task.state === 'queued' || task.state === 'downloading' || task.state === 'extracting'
  );
  const isTerminal = $derived(
    task.state === 'complete' || task.state === 'failed' || task.state === 'cancelled'
  );

  const progressPercent = $derived(Math.min(100, Math.max(0, task.percent)));
</script>

<div class="border-border bg-card flex flex-col gap-1.5 rounded-md border px-3 py-2.5">
  <div class="flex items-center gap-2">
    {#if task.state === 'queued'}
      <Clock size={14} class="text-muted-foreground shrink-0" />
    {:else if task.state === 'downloading' || task.state === 'extracting'}
      <Loader2 size={14} class="text-primary shrink-0 animate-spin" />
    {:else if task.state === 'complete'}
      <CheckCircle2 size={14} class="shrink-0 text-green-500" />
    {:else if task.state === 'failed'}
      <AlertCircle size={14} class="text-destructive shrink-0" />
    {:else}
      <X size={14} class="text-muted-foreground shrink-0" />
    {/if}

    <div class="min-w-0 flex-1">
      <p class="truncate text-sm font-medium">{task.name || task.uid}</p>
      <div class="text-muted-foreground flex items-center gap-2 text-xs">
        <span>{stateLabel}</span>
        {#if sizeLabel}
          <span>{sizeLabel}</span>
        {/if}
        {#if speedLabel}
          <span>{speedLabel}</span>
        {/if}
        {#if task.state === 'extracting' && task.totalFiles > 0}
          <span>{task.filesExtracted}/{task.totalFiles} files</span>
        {/if}
      </div>
    </div>

    {#if isActive && oncancel}
      <button
        onclick={() => oncancel(task.uid)}
        class="hover:bg-accent flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded"
        aria-label="Cancel download"
      >
        <X size={12} />
      </button>
    {:else if isTerminal && ondismiss}
      <button
        onclick={() => ondismiss(task.uid)}
        class="hover:bg-accent flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded"
        aria-label="Dismiss"
      >
        <X size={12} />
      </button>
    {/if}
  </div>

  {#if task.state === 'downloading' || task.state === 'extracting'}
    <div class="bg-secondary h-1.5 w-full overflow-hidden rounded-full">
      <div
        class="bg-primary h-full rounded-full transition-all duration-150 ease-out"
        style="width: {progressPercent}%"
      ></div>
    </div>
  {/if}

  {#if task.error}
    <p class="text-destructive text-xs">{task.error}</p>
  {/if}
</div>
