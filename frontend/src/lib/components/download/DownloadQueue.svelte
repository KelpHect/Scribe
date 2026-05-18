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
  const recentCount = $derived(downloads.recentDownloads.length);
  const retryableFailedCount = $derived(downloads.retryableFailedDownloads.length);
</script>

{#if hasTasks}
  <div
    class="border-border bg-popover fixed right-4 bottom-4 z-50 flex w-80 flex-col overflow-hidden rounded-lg border shadow-lg"
    aria-label="Task center"
  >
    <div class="border-border bg-muted/50 flex items-center gap-2 border-b px-3 py-2">
      <div class="min-w-0 flex-1">
        <p class="text-sm font-medium">Task Center</p>
        <p class="text-muted-foreground truncate text-[11px]">
          {activeCount} active · {recentCount} recent
          {#if retryableFailedCount > 0}
            · {retryableFailedCount} retryable
          {/if}
        </p>
      </div>

      <button
        type="button"
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
          type="button"
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
            type="button"
            onclick={() => downloads.cancelAllInstalls()}
            class="text-muted-foreground hover:text-destructive cursor-pointer text-xs"
          >
            Cancel all
          </button>
        </div>
      {:else if retryableFailedCount > 0}
        <div class="border-border flex items-center justify-between gap-2 border-t px-3 py-1.5">
          <span class="text-muted-foreground text-xs">
            {retryableFailedCount} failed {retryableFailedCount === 1 ? 'task' : 'tasks'}
          </span>
          <button
            type="button"
            onclick={() => downloads.retryFailedInstalls()}
            class="text-muted-foreground hover:text-foreground cursor-pointer text-xs"
          >
            Retry failed
          </button>
        </div>
      {/if}
    {/if}
  </div>
{/if}
