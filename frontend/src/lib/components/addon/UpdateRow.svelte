<svelte:options runes />

<script lang="ts">
  import ArrowUpCircle from 'lucide-svelte/icons/arrow-up-circle';
  import Download from 'lucide-svelte/icons/download';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import { Badge, Button } from '$lib/components/ui';
  import ProgressBar from '$lib/components/ui/ProgressBar.svelte';
  import { openExternalURL } from '$lib/services/runtime-service';
  import type { MatchedAddon } from '$lib/services/esoui-service';
  import type { TaskProgress } from '$lib/stores/downloads.svelte';
  import { formatBytes } from '$lib/utils';

  interface Props {
    match: MatchedAddon;
    iconUrl: string | null;
    isThumbnail: boolean;
    task?: TaskProgress;
    isUpdating: boolean;
    globalInstalling: boolean;
    onupdate: () => void;
  }

  const { match, iconUrl, isThumbnail, task, isUpdating, globalInstalling, onupdate }: Props = $props();

  async function openEsoui(e: MouseEvent) {
    e.preventDefault();
    const url = match.remote?.uiFileInfoUrl;
    if (!url) return;
    await openExternalURL(url);
  }
</script>

<div
  class="card-elevated border-border bg-card flex flex-col gap-1 rounded-lg border px-3 py-2.5"
>
  <div class="flex items-center gap-3">
    <div
      class="bg-secondary flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-md"
    >
      {#if iconUrl}
        <img
          src={iconUrl}
          alt=""
          aria-hidden="true"
          class={isThumbnail ? 'h-10 w-10 rounded-md object-cover' : 'h-6 w-6 object-contain'}
          loading="lazy"
        />
      {:else}
        <ArrowUpCircle size={20} class="text-destructive" />
      {/if}
    </div>

    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-2">
        <span class="truncate text-sm font-medium">{match.remote?.uiName ?? match.folderName}</span>
        <Badge variant="secondary">{match.localVersion}</Badge>
        <span class="text-muted-foreground text-xs">→</span>
        <Badge variant="destructive">{match.remoteVersion}</Badge>
      </div>
      <div class="text-muted-foreground mt-0.5 truncate text-xs">
        {match.remote?.uiAuthorName ?? 'Unknown Author'}
        {#if match.remote?.uiFileInfoUrl}
          · <a
            href={match.remote.uiFileInfoUrl}
            target="_blank"
            rel="noopener noreferrer"
            onclick={openEsoui}
            class="hover:text-foreground underline">ESOUI</a
          >
        {/if}
      </div>
    </div>

    <Button
      variant="outline"
      size="sm"
      onclick={onupdate}
      disabled={isUpdating || globalInstalling}
    >
      {#if isUpdating}
        <Loader2 size={14} class="animate-spin" />
        {#if task?.state === 'queued'}
          Queued
        {:else if task?.state === 'downloading'}
          {task.percent > 0 ? `${Math.round(task.percent)}%` : 'Downloading...'}
        {:else}
          Extracting...
        {/if}
      {:else}
        <Download size={14} />
        Update
      {/if}
    </Button>
  </div>

  {#if task && (task.state === 'downloading' || task.state === 'extracting')}
    <div class="mt-1 flex items-center gap-2 pl-[52px]">
      <ProgressBar percent={task.percent} class="flex-1" />
      {#if task.state === 'downloading' && task.speed > 0}
        <span class="text-muted-foreground shrink-0 text-[10px]">{formatBytes(task.speed)}/s</span>
      {/if}
      {#if task.state === 'extracting' && task.totalFiles > 0}
        <span class="text-muted-foreground shrink-0 text-[10px]"
          >{task.filesExtracted}/{task.totalFiles} files</span
        >
      {/if}
    </div>
  {/if}

  {#if task?.error}
    <p class="text-destructive pl-[52px] text-xs">{task.error}</p>
  {/if}
</div>
