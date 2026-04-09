<svelte:options runes />

<script lang="ts">
  import ChevronDown from 'lucide-svelte/icons/chevron-down';
  import ChevronUp from 'lucide-svelte/icons/chevron-up';
  import X from 'lucide-svelte/icons/x';
  import { getDownloadStore } from '$lib/stores';
  import DownloadItem from './DownloadItem.svelte';

  const downloads = getDownloadStore();

  let collapsed = $state(false);

  const hasTasks = $derived(downloads.tasks.length > 0);
  const activeCount = $derived(downloads.activeCount);
  const headerLabel = $derived(
    activeCount > 0
      ? `${activeCount} download${activeCount > 1 ? 's' : ''} in progress`
      : 'Downloads'
  );
</script>

{#if hasTasks}
  <div
    class="border-border bg-popover fixed right-4 bottom-4 z-50 flex w-80 flex-col overflow-hidden rounded-lg border shadow-lg"
  >
    <div class="border-border bg-muted/50 flex items-center gap-2 border-b px-3 py-2">
      <span class="flex-1 text-sm font-medium">{headerLabel}</span>

      <button
        onclick={() => (collapsed = !collapsed)}
        class="hover:bg-accent flex h-6 w-6 cursor-pointer items-center justify-center rounded"
        aria-label={collapsed ? 'Expand' : 'Collapse'}
      >
        {#if collapsed}
          <ChevronUp size={14} />
        {:else}
          <ChevronDown size={14} />
        {/if}
      </button>

      {#if !downloads.isDownloading}
        <button
          onclick={() => downloads.clearFinished()}
          class="hover:bg-accent flex h-6 w-6 cursor-pointer items-center justify-center rounded"
          aria-label="Clear all"
        >
          <X size={14} />
        </button>
      {/if}
    </div>

    {#if !collapsed}
      <div class="flex max-h-72 flex-col gap-1.5 overflow-y-auto p-2">
        {#each downloads.tasks as task (task.uid)}
          <DownloadItem
            {task}
            oncancel={(uid) => downloads.cancelInstall(uid)}
            ondismiss={(uid) => downloads.clearTask(uid)}
          />
        {/each}
      </div>

      {#if downloads.isDownloading}
        <div class="border-border border-t px-3 py-1.5">
          <button
            onclick={() => downloads.cancelAllInstalls()}
            class="text-muted-foreground hover:text-destructive cursor-pointer text-xs"
          >
            Cancel all
          </button>
        </div>
      {/if}
    {/if}
  </div>
{/if}
