<svelte:options runes />

<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Download from 'lucide-svelte/icons/download';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import RefreshCw from 'lucide-svelte/icons/refresh-cw';
  import { Badge, Button, Skeleton } from '$lib/components/ui';
  import { ErrorBoundary } from '$lib/components/ui';
  import { UpdateRow } from '$lib/components/addon';
  import { PageToolbar } from '$lib/components/layout';
  import { getRemoteStore, getDownloadStore } from '$lib/stores';
  import { _setUpdateCount } from '$lib/stores/remote.svelte';
  import {
    fetchCategories,
    fetchMatchedAddons,
    type MatchedAddon,
    type Category
  } from '$lib/services/esoui-service';
  import { categoriesQueryKey, matchedAddonsQueryKey } from '$lib/db/query-state';

  const remote = getRemoteStore();
  const downloads = getDownloadStore();

  const matchedQuery = createQuery(() => ({
    queryKey: matchedAddonsQueryKey,
    queryFn: async (): Promise<MatchedAddon[]> => fetchMatchedAddons()
  }));

  const categoriesQuery = createQuery(() => ({
    queryKey: categoriesQueryKey,
    queryFn: async (): Promise<Category[]> => fetchCategories()
  }));

  const matchedAddons = $derived((matchedQuery.data as MatchedAddon[]) ?? []);
  const categories = $derived((categoriesQuery.data as Category[]) ?? []);
  const isLoading = $derived(matchedQuery.isLoading);
  const error = $derived(matchedQuery.isError ? 'Failed to check for updates.' : null);

  const updatesAvailable = $derived(matchedAddons.filter((m: MatchedAddon) => m.updateAvailable));

  $effect(() => {
    _setUpdateCount(updatesAvailable.length);
  });

  async function updateOne(match: MatchedAddon) {
    if (!match.remote) return;
    try {
      await remote.install(match.remote.uid);
    } catch {}
  }

  async function updateAll() {
    const uids = updatesAvailable
      .map((m: MatchedAddon) => m.remote?.uid)
      .filter(Boolean) as string[];
    if (uids.length === 0) return;
    try {
      await remote.batchInstall(uids);
    } catch {}
  }

  function getTaskState(uid: string) {
    return downloads.getTask(uid);
  }

  const categoryIconMap = $derived(
    new Map<string, string>(categories.map((c: Category) => [c.id, c.iconUrl]))
  );

  function getAddonIconUrl(match: MatchedAddon): string | null {
    const fullImg = match.remote?.uiIMGs?.[0];
    if (fullImg) return fullImg;
    const catId = match.remote?.categoryId;
    if (!catId) return null;
    return categoryIconMap.get(catId) ?? null;
  }

  function getIsThumbnail(match: MatchedAddon): boolean {
    return !!match.remote?.uiIMGs?.[0];
  }
</script>

<div class="flex h-full flex-col">
  <PageToolbar title="Updates" subtitle="Available updates for your installed addons">
    {#snippet icon()}
      <Download size={14} class="text-[var(--color-toolbar-foreground)]" />
    {/snippet}

    {#snippet actions()}
      <button
        onclick={() => remote.forceRefresh()}
        disabled={remote.refreshing || isLoading}
        class="flex h-7 cursor-pointer items-center gap-1 rounded-md border border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] px-2 text-[11px] font-medium text-[var(--color-toolbar-foreground)] transition-colors hover:bg-[var(--color-toolbar-accent)] disabled:opacity-50"
      >
        <RefreshCw size={13} class={remote.refreshing ? 'animate-spin' : ''} />
        Refresh
      </button>
      <button
        onclick={updateAll}
        disabled={updatesAvailable.length === 0 ||
          isLoading ||
          remote.installing ||
          downloads.isDownloading}
        class="flex h-7 cursor-pointer items-center gap-1 rounded-md bg-[var(--color-toolbar-accent)] px-2.5 text-[11px] font-medium text-[var(--color-toolbar-foreground)] transition-colors hover:bg-[var(--color-toolbar-border)] disabled:opacity-50"
      >
        {#if downloads.isDownloading}
          <Loader2 size={13} class="animate-spin" />
          Downloading ({downloads.activeCount})
        {:else}
          <Download size={13} />
          Update All
        {/if}
      </button>
    {/snippet}
  </PageToolbar>

  <div class="flex min-h-0 flex-1 flex-col gap-2 px-4 pt-2.5 pb-3">
    <ErrorBoundary {error} onretry={() => remote.forceRefresh()}>
      {#snippet children()}
        {#if isLoading}
          <div class="flex flex-col gap-1.5">
            {#each { length: 4 } as _, i (i)}
              <div class="border-border bg-card flex items-center gap-3 rounded-lg border px-3 py-2.5">
                <Skeleton class="h-10 w-10 shrink-0 rounded-md" />
                <div class="flex flex-1 flex-col gap-1.5">
                  <Skeleton class="h-4 w-48" />
                  <Skeleton class="h-3 w-32" />
                </div>
                <Skeleton class="h-7 w-20 rounded-md" />
              </div>
            {/each}
          </div>
        {:else if updatesAvailable.length === 0}
          <div
            class="border-border flex flex-1 items-center justify-center rounded-lg border border-dashed p-8"
          >
            <div class="flex flex-col items-center gap-3 text-center">
              <div class="bg-muted flex h-16 w-16 items-center justify-center rounded-full">
                <Download size={28} class="text-muted-foreground" />
              </div>
              <h3 class="text-lg font-medium">All up to date</h3>
              <p class="text-muted-foreground max-w-sm text-sm">
                No updates available. Addon updates will appear here when newer versions are detected on
                ESOUI.
              </p>
            </div>
          </div>
        {:else}
          <div class="flex flex-col gap-1.5">
            {#each updatesAvailable as match (match.folderName)}
              {@const uid = match.remote?.uid ?? ''}
              {@const task = getTaskState(uid)}
              {@const isUpdating =
                task?.state === 'queued' ||
                task?.state === 'downloading' ||
                task?.state === 'extracting'}
              <UpdateRow
                {match}
                iconUrl={getAddonIconUrl(match)}
                isThumbnail={getIsThumbnail(match)}
                {task}
                {isUpdating}
                globalInstalling={remote.installing}
                onupdate={() => updateOne(match)}
              />
            {/each}
          </div>
        {/if}
      {/snippet}
    </ErrorBoundary>
  </div>
</div>
