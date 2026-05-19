<svelte:options runes />

<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import ArrowUpDown from 'lucide-svelte/icons/arrow-up-down';
  import AlertTriangle from 'lucide-svelte/icons/alert-triangle';
  import Calendar from 'lucide-svelte/icons/calendar';
  import Cpu from 'lucide-svelte/icons/cpu';
  import Download from 'lucide-svelte/icons/download';
  import ExternalLink from 'lucide-svelte/icons/external-link';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import Package from 'lucide-svelte/icons/package';
  import RefreshCw from 'lucide-svelte/icons/refresh-cw';
  import Search from 'lucide-svelte/icons/search';
  import Star from 'lucide-svelte/icons/star';
  import { Badge, Button, CategorySelect, Select, Skeleton } from '$lib/components/ui';
  import { PageToolbar } from '$lib/components/layout';
  import RemoteAddonDetail from '$lib/components/addon/RemoteAddonDetail.svelte';
  import FixedAddonImage from '$lib/components/addon/FixedAddonImage.svelte';
  import {
    openContextMenu,
    openContextMenuAt,
    type ContextMenuEntry
  } from '$lib/services/context-menu-service';
  import { openExternalURL } from '$lib/services/runtime-service';
  import { getRemoteStore, navigation } from '$lib/stores';
  import { getDownloadStore } from '$lib/stores/downloads.svelte';
  import {
    compareEsoUiCategoryOrder,
    formatCompact,
    getCategorySection,
    getCategoryIndentLevel
  } from '$lib/utils';
  import {
    buildRemoteCatalogIndex,
    filterRemoteCatalog,
    sortGameVersionsDescending
  } from '$lib/perf/remote-catalog-index';
  import {
    measureFrontendTiming,
    recordFrontendGauge
  } from '$lib/diagnostics/frontend-perf';
  import { getCatalogStatusView } from '$lib/catalog/status';
  import {
    fetchCategories,
    fetchMatchedAddons,
    fetchRemoteAddons,
    fetchRemoteCatalogStatus,
    type RemoteAddon,
    type MatchedAddon,
    type Category,
    type RemoteCatalogStatus
  } from '$lib/services/esoui-service';
  import { createQuery } from '@tanstack/svelte-query';
  import { createVirtualizer } from '@tanstack/svelte-virtual';
  import {
    remoteAddonsQueryKey,
    matchedAddonsQueryKey,
    categoriesQueryKey,
    refreshInstalledState
  } from '$lib/db/query-state';

  type CategorySection = 'Stand-Alone Addons' | 'Class & Role Specific' | 'Utilities' | 'Optional';

  type CategoryOption = {
    id: string;
    name: string;
    iconUrl?: string;
    count: number;
    section: CategorySection;
    indentLevel?: number;
  };

  const CATEGORY_SECTION_ORDER: CategorySection[] = [
    'Stand-Alone Addons',
    'Class & Role Specific',
    'Utilities',
    'Optional'
  ];

  const remote = getRemoteStore();
  const downloads = getDownloadStore();
  let searchValue = $state('');
  let selectedAddon = $state<RemoteAddon | null>(null);
  let detailOpen = $state(false);
  let searchInputEl = $state<HTMLInputElement | undefined>();

  let scrollEl = $state<HTMLDivElement | undefined>();
  let versionFilter = $state('');
  let contentFilter = $state<'all' | 'libraries'>('all');
  let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    if (navigation.pendingSearch) {
      searchValue = navigation.pendingSearch;
      remote.setSearch(navigation.pendingSearch);
      navigation.pendingSearch = '';
    }

    const focusSearch = () => searchInputEl?.focus();
    window.addEventListener('scribe:focus-search', focusSearch);

    const closeModal = () => (detailOpen = false);
    window.addEventListener('scribe:close-modal', closeModal);

    return () => {
      window.removeEventListener('scribe:focus-search', focusSearch);
      window.removeEventListener('scribe:close-modal', closeModal);
    };
  });

  const remoteAddonsQuery = createQuery(() => ({
    queryKey: remoteAddonsQueryKey,
    queryFn: async (): Promise<RemoteAddon[]> => fetchRemoteAddons()
  }));
  const matchedQuery = createQuery(() => ({
    queryKey: matchedAddonsQueryKey,
    queryFn: async (): Promise<MatchedAddon[]> => fetchMatchedAddons()
  }));
  const categoriesQuery = createQuery(() => ({
    queryKey: categoriesQueryKey,
    queryFn: async (): Promise<Category[]> => fetchCategories()
  }));
  const remoteStatusQuery = createQuery(() => ({
    queryKey: ['remote-catalog-status'] as const,
    queryFn: async (): Promise<RemoteCatalogStatus> => fetchRemoteCatalogStatus(),
    refetchInterval: 5000
  }));

  const remoteAddons = $derived((remoteAddonsQuery.data as RemoteAddon[]) ?? []);
  const matchedAddons = $derived((matchedQuery.data as MatchedAddon[]) ?? []);
  const categories = $derived((categoriesQuery.data as Category[]) ?? []);
  const remoteStatus = $derived((remoteStatusQuery.data as RemoteCatalogStatus | undefined) ?? null);
  const isLoading = $derived(remoteAddonsQuery.isLoading && remoteAddons.length === 0);
  const isError = $derived(remoteAddonsQuery.isError);
  const hasRemoteCatalogStatus = $derived(remoteStatusQuery.isSuccess && remoteStatus !== null);
  const catalogStatusView = $derived(
    getCatalogStatusView({
      remoteCount: remoteAddons.length,
      hasRemoteCatalogStatus,
      remoteStatus,
      isError
    })
  );
  const staleRefreshFailed = $derived(catalogStatusView === 'stale-refresh-failed');
  const refreshingCache = $derived(catalogStatusView === 'refreshing-cache');
  const showingStaleCache = $derived(catalogStatusView === 'showing-stale-cache');

  const categoryMap = $derived(new Map(categories.map((c: Category) => [c.id, c])));

  const indexedRemoteAddons = $derived(buildRemoteCatalogIndex(remoteAddons, categories));

  const installedUIDs = $derived(
    new Set(matchedAddons.map((m: MatchedAddon) => m.remote?.uid).filter(Boolean) as string[])
  );

  const matchedByUID = $derived(
    new Map(
      matchedAddons
        .filter((m: MatchedAddon) => !!m.remote?.uid)
        .map((m: MatchedAddon) => [m.remote!.uid, m])
    )
  );

  const availableVersions = $derived.by(() => {
    const versionNames = new Map<string, string>();
    for (const r of indexedRemoteAddons) {
      for (const cv of r.addon.compatabilities ?? []) {
        if (cv.version && !versionNames.has(cv.version)) {
          versionNames.set(cv.version, cv.name ?? cv.version);
        }
      }
    }
    return sortGameVersionsDescending(versionNames.entries());
  });

  const latestVersion = $derived(availableVersions[0] ?? null);

  const versionOptions = $derived([
    { value: '', label: 'All Versions' },
    ...(latestVersion
      ? [
          {
            value: latestVersion.version,
            label: latestVersion.name
              ? `Latest: ${latestVersion.name} (${latestVersion.version})`
              : `Latest Game Version (${latestVersion.version})`
          }
        ]
      : [])
  ]);
  const sortOptions = [
    { value: 'title', label: 'Sort By: Title' },
    { value: 'author', label: 'Sort By: Author' },
    { value: 'category', label: 'Sort By: Category' },
    { value: 'downloads', label: 'Sort By: Downloads' },
    { value: 'favorites', label: 'Sort By: Favorites' },
    { value: 'date', label: 'Sort By: Date' }
  ];
  const contentOptions = [
    { value: 'all', label: 'All Addons' },
    { value: 'libraries', label: 'Libraries & Dependencies' }
  ];

  const filterResult = $derived.by(() => {
    return measureFrontendTiming(
      'findMore.filterSort',
      () =>
        filterRemoteCatalog(indexedRemoteAddons, {
          query: remote.searchQuery,
          contentFilter,
          hideInstalled: remote.hideInstalled,
          installedUIDs,
          versionFilter,
          categoryFilter: remote.categoryFilter,
          sortKey: remote.sortBy,
          sortDirection: remote.sortDirection
        }),
      (result) => ({
        sourceCount: indexedRemoteAddons.length,
        resultCount: result.list.length,
        categoryCount: result.countMap.size,
        queryLength: result.query.length,
        categoryFilterCount: remote.categoryFilter.length,
        versionFiltered: !!versionFilter,
        contentFilter,
        hideInstalled: remote.hideInstalled,
        sortKey: result.sortKey,
        sortDirection: result.sortDirection
      })
    );
  });

  const filteredRemote = $derived(filterResult.list);
  const countsPerCategory = $derived(filterResult.countMap);

  const totalMatchingCount = $derived.by(() => {
    let total = 0;
    for (const v of countsPerCategory.values()) total += v;
    return total;
  });

  const visibleCategories = $derived(
    [...categories]
      .filter((c: Category) => (countsPerCategory.get(c.id) ?? 0) > 0)
      .sort(compareEsoUiCategoryOrder)
  );

  const categoryOptions = $derived.by(() => {
    return [...visibleCategories]
      .sort((a, b) => {
        const sectionA = CATEGORY_SECTION_ORDER.indexOf(getCategorySection(a, categoryMap));
        const sectionB = CATEGORY_SECTION_ORDER.indexOf(getCategorySection(b, categoryMap));
        if (sectionA !== sectionB) return sectionA - sectionB;
        return compareEsoUiCategoryOrder(a, b);
      })
      .map((c: Category) => ({
        id: c.id,
        name: c.name,
        iconUrl: c.iconUrl,
        count: countsPerCategory.get(c.id) ?? 0,
        section: getCategorySection(c, categoryMap),
        indentLevel: getCategoryIndentLevel(c, categoryMap)
      })) satisfies CategoryOption[];
  });

  const ITEM_HEIGHT = 62;
  const LIST_OVERSCAN = 6;

  const virtualizerStore = createVirtualizer({
    count: 0,
    getScrollElement: () => scrollEl ?? null,
    estimateSize: () => ITEM_HEIGHT,
    overscan: LIST_OVERSCAN
  });

  $effect(() => {
    const list = filteredRemote;
    const el = scrollEl;
    get(virtualizerStore).setOptions({
      count: list.length,
      getScrollElement: () => el ?? null,
      estimateSize: () => ITEM_HEIGHT,
      overscan: LIST_OVERSCAN,
      getItemKey: (index: number) => list[index]?.addon.uid ?? index
    });
  });

  $effect(() => {
    recordFrontendGauge('findMore.visibleItems', $virtualizerStore.getVirtualItems().length, {
      resultCount: filteredRemote.length,
      totalSize: $virtualizerStore.getTotalSize()
    });
  });

  function onSearch(e: Event) {
    const target = e.target as HTMLInputElement;
    searchValue = target.value;
    if (searchDebounceTimer !== null) clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(() => {
      searchDebounceTimer = null;
      remote.setSearch(target.value);
    }, 200);
  }

  function openDetail(addon: RemoteAddon) {
    selectedAddon = addon;
    detailOpen = true;
  }

  function getInstallTask(uid: string) {
    return downloads.getTask(uid);
  }

  async function installFromRow(e: MouseEvent, addon: RemoteAddon) {
    e.stopPropagation();
    await remote.install(addon.uid);
  }

  function openFindMoreContextMenu(e: MouseEvent, addon: RemoteAddon, alreadyInstalled: boolean) {
    const items: ContextMenuEntry[] = [
      { label: 'View Details', icon: Search, action: () => openDetail(addon) },
      { type: 'separator' },
      ...(!alreadyInstalled
        ? [
            {
              label: 'Install',
              icon: Download,
              disabled: remote.isInstallingUID(addon.uid),
              action: () => remote.install(addon.uid)
            }
          ]
        : []),
      ...(addon.uiFileInfoUrl
        ? [
            {
              label: 'Open Website',
              icon: ExternalLink,
              action: () => openExternalURL(addon.uiFileInfoUrl)
            }
          ]
        : [])
    ];
    openContextMenu(e, items);
  }

  function openFindMoreKeyboardMenu(
    e: KeyboardEvent,
    addon: RemoteAddon,
    alreadyInstalled: boolean
  ) {
    const target = e.currentTarget as HTMLElement | null;
    const rect = target?.getBoundingClientRect();
    const items: ContextMenuEntry[] = [
      { label: 'View Details', icon: Search, action: () => openDetail(addon) },
      ...(addon.uiFileInfoUrl
        ? [{ label: 'Open ESOUI', icon: ExternalLink, action: () => openExternalURL(addon.uiFileInfoUrl) }]
        : []),
      { type: 'separator' },
      {
        label: alreadyInstalled ? 'Already Installed' : 'Install',
        icon: Download,
        disabled: alreadyInstalled || remote.isInstallingUID(addon.uid),
        action: () => remote.install(addon.uid)
      }
    ];
    openContextMenuAt(rect ? rect.left + 12 : 0, rect ? rect.top + 12 : 0, items);
  }

  function handleRemoteRowKeydown(e: KeyboardEvent, addon: RemoteAddon, alreadyInstalled: boolean) {
    if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
      e.preventDefault();
      openFindMoreKeyboardMenu(e, addon, alreadyInstalled);
      return;
    }

    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      openDetail(addon);
    }
  }

  const selectedInstalledFolderName = $derived(
    selectedAddon ? (matchedByUID.get(selectedAddon.uid)?.folderName ?? null) : null
  );

  const selectedMatch = $derived(selectedAddon ? (matchedByUID.get(selectedAddon.uid) ?? null) : null);

  const selectedCategory = $derived(
    selectedAddon ? (categoryMap.get(selectedAddon.categoryId) ?? null) : null
  );
</script>

<div class="flex h-full flex-col">
  <PageToolbar title="Find More" subtitle="Browse and discover addons from ESOUI">
    {#snippet icon()}
      <Search size={14} class="text-[var(--color-toolbar-foreground)]" />
    {/snippet}

    {#snippet actions()}
      <label class="flex cursor-pointer items-center gap-1.5 select-none">
        <button
          role="switch"
          aria-checked={remote.hideInstalled}
          aria-label="Hide installed addons"
          onclick={() => remote.setHideInstalled(!remote.hideInstalled)}
          class="relative inline-flex h-4 w-7 shrink-0 cursor-pointer items-center rounded-full border border-[var(--color-toolbar-border)] transition-colors {remote.hideInstalled
            ? 'bg-primary'
            : 'bg-[var(--color-toolbar-input)]'}"
        >
          <span
            class="pointer-events-none inline-block h-3 w-3 rounded-full bg-white shadow-sm transition-transform {remote.hideInstalled
              ? 'translate-x-[13px]'
              : 'translate-x-[2px]'}"
          ></span>
        </button>
        <span class="text-[11px] whitespace-nowrap text-[var(--color-toolbar-muted)]">
          Hide installed
        </span>
      </label>

      <button
        onclick={() => remote.forceRefresh()}
        disabled={remote.refreshing}
        class="flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded-md border border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] text-[var(--color-toolbar-muted)] transition-colors hover:bg-[var(--color-toolbar-accent)] hover:text-[var(--color-toolbar-foreground)] disabled:opacity-50"
        aria-label="Refresh addon list"
      >
        <RefreshCw size={13} class={remote.refreshing ? 'animate-spin' : ''} />
      </button>
      <button
        onclick={() => remote.setSortDirection(remote.sortDirection === 'asc' ? 'desc' : 'asc')}
        class="flex h-7 shrink-0 cursor-pointer items-center gap-1 rounded-md border border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] px-2 text-[11px] font-medium text-[var(--color-toolbar-foreground)] transition-colors hover:bg-[var(--color-toolbar-accent)]"
        aria-label="Toggle sort direction"
      >
        <ArrowUpDown size={12} />
        {remote.sortDirection === 'asc' ? 'Asc' : 'Desc'}
      </button>
    {/snippet}

    {#snippet filters()}
      <div class="relative min-w-[220px] flex-1">
        <Search
          size={14}
          class="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-[var(--color-toolbar-input-placeholder)]"
        />
        <input
          bind:this={searchInputEl}
          placeholder="Search ESOUI for addons…"
          class="h-7 w-full rounded-md border border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] pr-2.5 pl-7 text-xs text-[var(--color-toolbar-input-foreground)] placeholder:text-[var(--color-toolbar-input-placeholder)] focus:outline-none"
          value={searchValue}
          oninput={onSearch}
        />
      </div>

      <CategorySelect
        dark
        class="w-[220px] shrink-0"
        value={remote.categoryFilter}
        options={categoryOptions}
        totalCount={totalMatchingCount}
        onchange={(ids: string[]) => remote.setCategoryFilter(ids)}
      />

      <Select
        dark
        class="w-[175px] shrink-0"
        value={contentFilter}
        options={contentOptions}
        onchange={(v) => (contentFilter = v as 'all' | 'libraries')}
        placeholder="Content"
        aria-label="Filter addon content"
      />

      <Select
        dark
        class="w-[170px] shrink-0"
        value={versionFilter}
        options={versionOptions}
        onchange={(v) => (versionFilter = v)}
        placeholder="Game Version"
        aria-label="Filter by game version"
      />

      <Select
        dark
        class="w-[180px] shrink-0"
        value={remote.sortBy}
        options={sortOptions}
        onchange={(v) =>
          remote.setSortBy(
            v as 'title' | 'author' | 'category' | 'downloads' | 'favorites' | 'date'
          )}
        placeholder="Sort by"
        aria-label="Sort by"
        align="end"
        menuClass="w-56"
      />
    {/snippet}
  </PageToolbar>

  <div class="flex min-h-0 flex-1 flex-col gap-2 px-4 pt-2.5 pb-3">
    {#if isError}
      <div
        class="border-destructive/50 bg-destructive/10 flex items-center justify-between gap-3 rounded-lg border p-3"
      >
        <span class="text-destructive text-sm">Failed to load addons from ESOUI.</span>
        <Button variant="outline" size="sm" onclick={() => remote.forceRefresh()}>
          <RefreshCw size={13} />
          Retry
        </Button>
      </div>
    {/if}

    {#if isLoading}
      <div class="flex flex-col gap-1.5">
        {#each { length: 8 } as _, i (i)}
          <div class="flex items-center gap-3 rounded-lg border px-3 py-2.5">
            <Skeleton class="h-10 w-10 shrink-0 rounded-md" />
            <div class="flex flex-1 flex-col gap-1.5">
              <Skeleton class="h-4 w-48" />
              <Skeleton class="h-3 w-32" />
            </div>
          </div>
        {/each}
      </div>
    {:else}
      {#if filteredRemote.length === 0}
        <div class="min-h-0 flex-1">
          <div
            class="border-border flex h-full items-center justify-center rounded-lg border border-dashed p-8"
          >
            <div class="flex flex-col items-center gap-3 text-center">
              <div class="bg-muted flex h-16 w-16 items-center justify-center rounded-full">
                {#if searchValue || remote.categoryFilter.length > 0 || versionFilter}
                  <Search size={28} class="text-muted-foreground" />
                {:else}
                  <Package size={28} class="text-muted-foreground" />
                {/if}
              </div>
              {#if searchValue || remote.categoryFilter.length > 0 || versionFilter}
                <h3 class="text-lg font-medium">No results</h3>
                <p class="text-muted-foreground max-w-sm text-sm">
                  Try a different search term, category, version, or content filter.
                </p>
              {:else}
                {#if catalogStatusView === 'no-cache'}
                  <h3 class="text-lg font-medium">No cached addon data available</h3>
                  <p class="text-muted-foreground max-w-sm text-sm">
                    Scribe could not load ESOUI and has no saved catalog to show. Check your
                    internet connection and try refreshing.
                  </p>
                {:else}
                  <h3 class="text-lg font-medium">No addons loaded</h3>
                  <p class="text-muted-foreground max-w-sm text-sm">
                    Scribe has not loaded any ESOUI catalog data yet. Try refreshing the addon list.
                  </p>
                {/if}
                <Button variant="outline" onclick={() => remote.forceRefresh()}>
                  <RefreshCw size={14} />
                  Try Again
                </Button>
              {/if}
            </div>
          </div>
        </div>
      {:else}
        {#if refreshingCache}
          <div
            class="border-border bg-muted/40 text-muted-foreground flex items-center justify-between gap-3 rounded-lg border p-3 text-sm"
          >
            <div class="flex min-w-0 items-center gap-2">
              <Loader2 size={16} class="shrink-0 animate-spin" />
              <span>Showing cached ESOUI data while a background refresh is running.</span>
            </div>
            <Button variant="ghost" size="sm" disabled>
              Refreshing
            </Button>
          </div>
        {:else if staleRefreshFailed}
          <div
            class="border-amber-500/40 bg-amber-500/10 flex items-center justify-between gap-3 rounded-lg border p-3"
          >
            <div class="flex min-w-0 items-start gap-2">
              <AlertTriangle size={16} class="mt-0.5 shrink-0 text-amber-600 dark:text-amber-400" />
              <div class="min-w-0">
                <p class="text-sm font-medium text-amber-900 dark:text-amber-100">
                  Showing cached ESOUI data
                </p>
                <p class="text-xs text-amber-800 dark:text-amber-200">
                  Background refresh failed. Check your connection or try refreshing.
                </p>
              </div>
            </div>
            <Button variant="outline" size="sm" onclick={() => remote.forceRefresh()}>
              <RefreshCw size={13} />
              Retry
            </Button>
          </div>
        {:else if showingStaleCache}
          <div
            class="border-border bg-muted/40 text-muted-foreground flex items-center justify-between gap-3 rounded-lg border p-3 text-sm"
          >
            <span>Showing cached ESOUI data while Scribe refreshes in the background.</span>
            <Button variant="ghost" size="sm" onclick={() => remote.forceRefresh()}>
              <RefreshCw size={13} />
              Refresh
            </Button>
          </div>
        {/if}

        <p class="text-muted-foreground px-1 text-[11px]">
          {filteredRemote.length.toLocaleString()} results
        </p>
        <div bind:this={scrollEl} class="min-h-0 flex-1 overflow-y-auto" style="contain: strict;">
          <div
            style="height: {$virtualizerStore.getTotalSize()}px; width: 100%; position: relative;"
          >
            <div
              style="position: absolute; top: 0; left: 0; width: 100%; transform: translateY({$virtualizerStore.getVirtualItems()[0]
                ?.start ?? 0}px);"
            >
              {#each $virtualizerStore.getVirtualItems() as virtualItem (virtualItem.key)}
                {@const item = filteredRemote[virtualItem.index]}
                {#if item}
                  {@const addon = item.addon}
                  {@const alreadyInstalled = installedUIDs.has(addon.uid)}
                  {@const iconUrl = item.listIconUrl}
                  {@const isThumbnail = item.listIconIsThumbnail}
                  {@const category = item.category}
                  {@const compatVersion = item.latestCompatibilityVersion}
                  {@const compatName = item.latestCompatibilityName}
                  {@const installTask = getInstallTask(addon.uid)}
                  {@const rowInstalling =
                    remote.isInstallingUID(addon.uid) ||
                    installTask?.state === 'queued' ||
                    installTask?.state === 'downloading' ||
                    installTask?.state === 'extracting'}
                  <div data-index={virtualItem.index} style="padding-bottom: 6px;">
                    <div
                      class="card-interactive border-border bg-card flex items-center gap-3 rounded-lg border px-3 py-2.5"
                      onclick={() => openDetail(addon)}
                      oncontextmenu={(e) => openFindMoreContextMenu(e, addon, alreadyInstalled)}
                      onkeydown={(e) => handleRemoteRowKeydown(e, addon, alreadyInstalled)}
                      role="button"
                      tabindex="0"
                      aria-label={`View details for ${addon.uiName}`}
                    >
                      <FixedAddonImage src={iconUrl} thumbnail={isThumbnail} />

                      <div class="min-w-0 flex-1">
                        <div class="flex items-center gap-2">
                          <span class="truncate text-sm font-medium">{addon.uiName}</span>
                          {#if addon.uiVersion}
                            <Badge variant="secondary">{addon.uiVersion}</Badge>
                          {/if}
                          {#if alreadyInstalled}
                            <Badge variant="outline">Installed</Badge>
                          {/if}
                          {#if item.updatedState === 'today'}
                            <Badge
                              variant="destructive"
                              class="px-2 py-0.5 text-[11px] font-semibold uppercase"
                            >
                              Updated
                            </Badge>
                          {:else if item.updatedState === 'recent'}
                            <Badge
                              variant="outline"
                              class="border-green-700/40 bg-green-600/90 px-2 py-0.5 text-[11px] font-semibold text-white uppercase dark:border-green-500/40 dark:bg-green-500/90 dark:text-black"
                            >
                              Updated
                            </Badge>
                          {/if}
                        </div>
                        <div class="text-muted-foreground mt-0.5 flex items-center gap-2 text-xs">
                          <span class="truncate">{addon.uiAuthorName || 'Unknown Author'}</span>
                          {#if addon.uiDownloadTotal > 0}
                            <span class="flex shrink-0 items-center gap-0.5">
                              <Download size={10} />
                              {formatCompact(addon.uiDownloadTotal)}
                            </span>
                          {/if}
                          {#if addon.uiFavoriteTotal > 0}
                            <span class="flex shrink-0 items-center gap-0.5">
                              <Star size={10} />
                              {formatCompact(addon.uiFavoriteTotal)}
                            </span>
                          {/if}
                          {#if category}
                            <span class="flex min-w-0 items-center gap-1">
                              {#if category.iconUrl}
                                <img
                                  src={category.iconUrl}
                                  alt=""
                                  aria-hidden="true"
                                  width="14"
                                  height="14"
                                  class="h-3.5 w-3.5 shrink-0 object-contain"
                                  loading="lazy"
                                  decoding="async"
                                  draggable="false"
                                  referrerpolicy="no-referrer"
                                />
                              {/if}
                              <span class="truncate">{category.name}</span>
                            </span>
                          {/if}
                          {#if compatVersion}
                            <span
                              class="flex shrink-0 items-center gap-1"
                              title={compatName || compatVersion}
                            >
                              <Cpu size={10} />
                              v{compatVersion}
                            </span>
                          {/if}
                          {#if addon.uiDate}
                            <span class="flex shrink-0 items-center gap-1">
                              <Calendar size={10} />
                              {addon.uiDate}
                            </span>
                          {/if}
                        </div>
                      </div>

                      {#if !alreadyInstalled}
                        <button
                          type="button"
                          onclick={(e) => installFromRow(e, addon)}
                          disabled={rowInstalling}
                          class="border-border bg-background hover:bg-accent text-foreground flex h-8 shrink-0 items-center gap-1.5 rounded-md border px-3 text-xs font-medium transition-colors disabled:cursor-default disabled:opacity-60"
                          aria-label="Install addon"
                        >
                          {#if installTask?.state === 'queued'}
                            <Loader2 size={12} class="animate-spin" />
                            Queued
                          {:else if installTask?.state === 'downloading'}
                            <Loader2 size={12} class="animate-spin" />
                            {installTask.percent > 0
                              ? `${Math.round(installTask.percent)}%`
                              : 'Downloading'}
                          {:else if installTask?.state === 'extracting'}
                            <Loader2 size={12} class="animate-spin" />
                            Extracting
                          {:else if rowInstalling}
                            <Loader2 size={12} class="animate-spin" />
                            Installing
                          {:else}
                            <Download size={12} />
                            Install
                          {/if}
                        </button>
                      {/if}
                    </div>
                  </div>
                {/if}
              {/each}
            </div>
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>

<RemoteAddonDetail
  addon={selectedAddon}
  open={detailOpen}
  onclose={() => (detailOpen = false)}
  installedFolderName={selectedInstalledFolderName}
  category={selectedCategory}
  updateAvailable={selectedMatch?.updateAvailable ?? false}
  localVersion={selectedMatch?.localVersion ?? ''}
  updateState={selectedMatch?.updateState ?? ''}
  updateReason={selectedMatch?.updateReason ?? ''}
  oninstalled={() => refreshInstalledState()}
/>
