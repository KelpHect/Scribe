<svelte:options runes />

<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { SvelteSet } from 'svelte/reactivity';
  import { createQuery } from '@tanstack/svelte-query';
  import { createVirtualizer } from '@tanstack/svelte-virtual';
  import Download from 'lucide-svelte/icons/download';
  import ExternalLink from 'lucide-svelte/icons/external-link';
  import FolderOpen from 'lucide-svelte/icons/folder-open';
  import Package from 'lucide-svelte/icons/package';
  import RefreshCw from 'lucide-svelte/icons/refresh-cw';
  import Search from 'lucide-svelte/icons/search';
  import Trash2 from 'lucide-svelte/icons/trash-2';
  import { Badge, Skeleton } from '$lib/components/ui';
  import { ErrorBoundary } from '$lib/components/ui';
  import { PageToolbar } from '$lib/components/layout';
  import AddonCard from '$lib/components/addon/AddonCard.svelte';
  import AddonDetail from '$lib/components/addon/AddonDetail.svelte';
  import { CategoryHeader, MissingDepsBanner } from '$lib/components/addon';
  import { openContextMenu, type ContextMenuEntry } from '$lib/services/context-menu-service';
  import { fetchAddonPath, fetchInstalledAddons, openPath, type Addon } from '$lib/services/addon-service';
  import { openExternalURL } from '$lib/services/runtime-service';
  import { getRemoteStore } from '$lib/stores';
  import { batchInstall } from '$lib/stores/downloads.svelte';
  import { _setUpdateCount } from '$lib/stores/remote.svelte';
  import { uninstallRemoteAddon } from '$lib/stores/remote-mutations.svelte';
  import {
    type MatchedAddon,
    type Category,
    type RemoteAddon,
    type MissingDepInfo,
    fetchCategories,
    fetchMatchedAddons,
    fetchRemoteAddons,
    fetchMissingDependencies
  } from '$lib/services/esoui-service';
  import {
    addonPathQueryKey,
    categoriesQueryKey,
    installedAddonsQueryKey,
    matchedAddonsQueryKey,
    remoteAddonsQueryKey,
    refreshInstalledState,
  } from '$lib/db/query-state';

  const remote = getRemoteStore();
  let searchValue = $state('');
  let searchQuery = $state('');
  let selectedAddon = $state<Addon | null>(null);
  let detailOpen = $state(false);
  let searchInputEl = $state<HTMLInputElement | undefined>();

  let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

  function onSearch(e: Event) {
    const target = e.target as HTMLInputElement;
    searchValue = target.value;
    if (searchDebounceTimer !== null) clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(() => {
      searchDebounceTimer = null;
      searchQuery = searchValue;
    }, 200);
  }

  let missingDeps = $state<MissingDepInfo[]>([]);
  let dismissedRequiredDeps = $state(false);
  let dismissedOptionalDeps = $state(false);
  let batchInstalling = $state(false);

  const expandedCategories = new SvelteSet<string>();
  const expandedCategoriesStorageKey = 'scribe:installed:expanded-categories';
  let initialized = $state(false);

  async function checkMissingDeps() {
    dismissedRequiredDeps = false;
    dismissedOptionalDeps = false;
    try {
      missingDeps = await fetchMissingDependencies();
    } catch {}
  }

  async function installMissingDeps(optional: boolean) {
    const uids = missingDeps.filter((d) => d.canInstall && d.optional === optional).map((d) => d.remoteUID);
    if (uids.length === 0) return;
    batchInstalling = true;
    try {
      await batchInstall(uids);
      missingDeps = missingDeps.filter((d) => d.optional !== optional);
    } finally {
      batchInstalling = false;
    }
  }

  const installableRequiredDeps = $derived(missingDeps.filter((d) => d.canInstall && !d.optional));
  const installableOptionalDeps = $derived(missingDeps.filter((d) => d.canInstall && d.optional));

  onMount(() => {
    void checkMissingDeps();

    const focusSearch = () => searchInputEl?.focus();
    window.addEventListener('scribe:focus-search', focusSearch);

    const closeModal = () => (detailOpen = false);
    window.addEventListener('scribe:close-modal', closeModal);

    return () => {
      window.removeEventListener('scribe:focus-search', focusSearch);
      window.removeEventListener('scribe:close-modal', closeModal);
    };
  });

  const installedQuery = createQuery(() => ({
    queryKey: installedAddonsQueryKey,
    queryFn: async (): Promise<Addon[]> => fetchInstalledAddons()
  }));
  const matchedQuery = createQuery(() => ({
    queryKey: matchedAddonsQueryKey,
    queryFn: async (): Promise<MatchedAddon[]> => fetchMatchedAddons()
  }));
  const categoriesQuery = createQuery(() => ({
    queryKey: categoriesQueryKey,
    queryFn: async (): Promise<Category[]> => fetchCategories()
  }));
  const remoteAddonsQuery = createQuery(() => ({
    queryKey: remoteAddonsQueryKey,
    queryFn: async (): Promise<RemoteAddon[]> => fetchRemoteAddons()
  }));
  const addonPathQuery = createQuery(() => ({
    queryKey: addonPathQueryKey,
    queryFn: async (): Promise<string> => fetchAddonPath()
  }));

  const addons = $derived((installedQuery.data as Addon[]) ?? []);
  const matchedAddons = $derived((matchedQuery.data as MatchedAddon[]) ?? []);
  const categories = $derived((categoriesQuery.data as Category[]) ?? []);
  const remoteAddons = $derived((remoteAddonsQuery.data as RemoteAddon[]) ?? []);
  const addonPath = $derived((addonPathQuery.data as string) ?? '');
  const loading = $derived(installedQuery.isLoading && addons.length === 0);
  const error = $derived.by(() => {
    if (installedQuery.isError) return 'Failed to load installed addons';
    return addonPathQuery.error instanceof Error ? addonPathQuery.error.message : null;
  });
  const filteredAddons = $derived.by(() => {
    const q = searchQuery.toLowerCase().trim();
    if (!q) return addons;
    return addons.filter(
      (a) =>
        a.title.toLowerCase().includes(q) ||
        a.author.toLowerCase().includes(q) ||
        a.folderName.toLowerCase().includes(q)
    );
  });
  const libraryCount = $derived(addons.filter((a) => a.isLibrary).length);
  const totalCount = $derived(addons.length);

  const updatesAvailable = $derived(matchedAddons.filter((m: MatchedAddon) => m.updateAvailable));

  $effect(() => {
    _setUpdateCount(updatesAvailable.length);
  });

  function openDetail(addon: Addon) {
    selectedAddon = addon;
    detailOpen = true;
  }

  function openInstalledContextMenu(e: MouseEvent, addon: Addon) {
    const matched = matchedMap.get(addon.folderName.toLowerCase()) ?? null;
    const items: ContextMenuEntry[] = [
      { label: 'View Details', icon: Search, action: () => openDetail(addon) },
      { type: 'separator' },
      { label: 'Open Folder', icon: FolderOpen, action: () => openPath(addon.path) },
      ...(matched?.remote?.uiFileInfoUrl
        ? [{ label: 'Open ESOUI', icon: ExternalLink, action: () => openExternalURL(matched.remote!.uiFileInfoUrl) }]
        : []),
      ...(matched?.updateAvailable && matched.remote?.uid
        ? [{ label: 'Update', icon: Download, action: () => remote.install(matched.remote!.uid) }]
        : []),
      { type: 'separator' },
      {
        label: 'Uninstall',
        icon: Trash2,
        variant: 'destructive',
        action: async () => {
          await uninstallRemoteAddon(addon.folderName, addon.title);
          void refreshInstalledState();
          void checkMissingDeps();
        }
      }
    ];
    openContextMenu(e, items);
  }

  const updateSet = $derived(
    new Set(updatesAvailable.map((m: MatchedAddon) => m.folderName.toLowerCase()))
  );

  function hasUpdate(addon: Addon): boolean {
    return updateSet.has(addon.folderName.toLowerCase());
  }

  const matchedMap = $derived(
    new Map(matchedAddons.map((m: MatchedAddon) => [m.folderName.toLowerCase(), m]))
  );

  const categoryMap = $derived(
    new Map<string, Category>(categories.map((c: Category) => [c.id, c]))
  );

  const libraryCategoryIconUrl = $derived.by(() => {
    for (const addon of filteredAddons) {
      if (!addon.isLibrary) continue;
      const matched = matchedMap.get(addon.folderName.toLowerCase());
      const categoryId = matched?.remote?.categoryId;
      if (!categoryId) continue;
      const iconUrl = categoryMap.get(categoryId)?.iconUrl;
      if (iconUrl) return iconUrl;
    }
    return categories.find((c: Category) => /lib/i.test(c.name))?.iconUrl;
  });

  const depUIDMap = $derived.by(() => {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const map = new Map<string, string>();
    for (const r of remoteAddons) {
      for (const dir of r.uiDirs ?? []) {
        const key = dir.toLowerCase();
        if (!map.has(key)) map.set(key, r.uid);
      }
      if (r.uiName) {
        const key = r.uiName.toLowerCase();
        if (!map.has(key)) map.set(key, r.uid);
      }
    }
    return map;
  });

  function getCategoryIconUrl(addon: Addon): string | undefined {
    const matched = matchedMap.get(addon.folderName.toLowerCase());
    const categoryId = matched?.remote?.categoryId;
    const categoryIcon = categoryId ? categoryMap.get(categoryId)?.iconUrl : undefined;
    if (addon.isLibrary) return categoryIcon || undefined;
    const fullImg = matched?.remote?.uiIMGs?.[0];
    if (fullImg) return fullImg;
    return categoryIcon || undefined;
  }

  function getIsThumbnail(addon: Addon): boolean {
    if (addon.isLibrary) return false;
    const matched = matchedMap.get(addon.folderName.toLowerCase());
    return !!matched?.remote?.uiIMGs?.[0];
  }

  const installedFolderNamesSet = $derived(new Set(addons.map((a) => a.folderName.toLowerCase())));

  interface CategoryGroup {
    id: string;
    name: string;
    iconUrl: string;
    addons: Addon[];
  }

  const UNCATEGORIZED_ID = '__uncategorized__';
  const LIBRARIES_ID = '__libraries__';

  const groupedAddons = $derived.by(() => {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const groupMap = new Map<string, Addon[]>();
    const libraries: Addon[] = [];

    for (const addon of filteredAddons) {
      if (addon.isLibrary) {
        libraries.push(addon);
        continue;
      }

      const matched = matchedMap.get(addon.folderName.toLowerCase());
      const catId = matched?.remote?.categoryId;

      if (catId && categoryMap.has(catId)) {
        const existing = groupMap.get(catId);
        if (existing) {
          existing.push(addon);
        } else {
          groupMap.set(catId, [addon]);
        }
      } else {
        const existing = groupMap.get(UNCATEGORIZED_ID);
        if (existing) {
          existing.push(addon);
        } else {
          groupMap.set(UNCATEGORIZED_ID, [addon]);
        }
      }
    }

    const sortAddons = (a: Addon, b: Addon) => a.title.localeCompare(b.title);

    const groups: CategoryGroup[] = [];

    const namedGroups: CategoryGroup[] = [];
    for (const [catId, addons] of groupMap) {
      if (catId === UNCATEGORIZED_ID) continue;
      const cat = categoryMap.get(catId);
      if (!cat) continue;
      namedGroups.push({
        id: catId,
        name: cat.name,
        iconUrl: cat.iconUrl,
        addons: addons.sort(sortAddons)
      });
    }
    namedGroups.sort((a, b) => a.name.localeCompare(b.name));
    groups.push(...namedGroups);

    const uncategorized = groupMap.get(UNCATEGORIZED_ID);
    if (uncategorized && uncategorized.length > 0) {
      groups.push({
        id: UNCATEGORIZED_ID,
        name: 'Uncategorized',
        iconUrl: '',
        addons: uncategorized.sort(sortAddons)
      });
    }

      if (libraries.length > 0) {
        groups.push({
          id: LIBRARIES_ID,
          name: 'Libraries',
          iconUrl: libraryCategoryIconUrl ?? '',
          addons: libraries.sort(sortAddons)
        });
      }

    return groups;
  });

  $effect(() => {
    if (!initialized && groupedAddons.length > 0) {
      let restored = false;
      try {
        const raw = localStorage.getItem(expandedCategoriesStorageKey);
        if (raw) {
          const saved = JSON.parse(raw) as string[];
          const valid = new Set(groupedAddons.map((group) => group.id));
          for (const id of saved) {
            if (valid.has(id)) {
              expandedCategories.add(id);
              restored = true;
            }
          }
        }
      } catch {}

      if (!restored) {
        for (const group of groupedAddons) {
          expandedCategories.add(group.id);
        }
      }
      initialized = true;
    }
  });

  $effect(() => {
    if (!initialized) return;
    localStorage.setItem(expandedCategoriesStorageKey, JSON.stringify(Array.from(expandedCategories)));
  });

  function toggleCategory(id: string) {
    if (expandedCategories.has(id)) {
      expandedCategories.delete(id);
    } else {
      expandedCategories.add(id);
    }
  }

  function expandAll() {
    for (const group of groupedAddons) {
      expandedCategories.add(group.id);
    }
  }

  function collapseAll() {
    expandedCategories.clear();
  }

  type FlatRow =
    | { type: 'header'; group: CategoryGroup }
    | { type: 'addon'; addon: Addon; groupId: string };

  const HEADER_HEIGHT = 40;
  const ADDON_HEIGHT = 80;

  const flatRows = $derived.by((): FlatRow[] => {
    const rows: FlatRow[] = [];
    for (const group of groupedAddons) {
      rows.push({ type: 'header', group });
      if (expandedCategories.has(group.id)) {
        for (const addon of group.addons) {
          rows.push({ type: 'addon', addon, groupId: group.id });
        }
      }
    }
    return rows;
  });

  let scrollEl = $state<HTMLDivElement | undefined>();

  const virtualizerStore = createVirtualizer({
    count: 0,
    getScrollElement: () => scrollEl ?? null,
    estimateSize: () => ADDON_HEIGHT,
    overscan: 8
  });

  $effect(() => {
    const rows = flatRows;
    const el = scrollEl;
    void get(virtualizerStore).setOptions({
      count: rows.length,
      getScrollElement: () => el ?? null,
      estimateSize: (index: number) => {
        const row = rows[index];
        return row?.type === 'header' ? HEADER_HEIGHT : ADDON_HEIGHT;
      },
      overscan: 8,
      getItemKey: (index: number) => {
        const row = rows[index];
        if (!row) return index;
        return row.type === 'header' ? `h:${row.group.id}` : `a:${row.addon.id}`;
      }
    });
  });
</script>

<div class="flex h-full flex-col">
  <PageToolbar title="Installed Addons" subtitle="Manage your installed addons">
    {#snippet icon()}
      <Package size={14} class="text-[var(--color-toolbar-foreground)]" />
    {/snippet}

    {#snippet actions()}
      <Badge
        variant="secondary"
        class="border border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] text-[var(--color-toolbar-muted)]"
      >
        {totalCount} addon{totalCount === 1 ? '' : 's'}
        {#if libraryCount > 0}
          · {libraryCount} lib{libraryCount === 1 ? '' : 's'}
        {/if}
      </Badge>
      {#if remote.updateCount > 0}
        <Badge variant="destructive" class="border border-[var(--color-toolbar-border)]">
          {remote.updateCount} update{remote.updateCount === 1 ? '' : 's'}
        </Badge>
      {/if}
      <button
        onclick={() => {
          void refreshInstalledState();
          void checkMissingDeps();
        }}
        class="flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded-md border border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] text-[var(--color-toolbar-muted)] transition-colors hover:bg-[var(--color-toolbar-accent)] hover:text-[var(--color-toolbar-foreground)]"
        aria-label="Refresh addons"
      >
        <RefreshCw size={13} />
      </button>
    {/snippet}

    {#snippet filters()}
      <div class="relative flex-1">
        <Search
          size={14}
          class="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-[var(--color-toolbar-input-placeholder)]"
        />
        <input
          bind:this={searchInputEl}
          placeholder="Search installed addons…"
          class="h-7 w-full rounded-md border border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] pr-2.5 pl-7 text-xs text-[var(--color-toolbar-input-foreground)] placeholder:text-[var(--color-toolbar-input-placeholder)] focus:outline-none"
          value={searchValue}
          oninput={onSearch}
        />
      </div>

      <button
        onclick={expandAll}
        class="flex h-7 cursor-pointer items-center gap-1 rounded-md border border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] px-2 text-[11px] font-medium text-[var(--color-toolbar-muted)] transition-colors hover:bg-[var(--color-toolbar-accent)] hover:text-[var(--color-toolbar-foreground)]"
        aria-label="Expand all categories"
      >
        Expand All
      </button>
      <button
        onclick={collapseAll}
        class="flex h-7 cursor-pointer items-center gap-1 rounded-md border border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] px-2 text-[11px] font-medium text-[var(--color-toolbar-muted)] transition-colors hover:bg-[var(--color-toolbar-accent)] hover:text-[var(--color-toolbar-foreground)]"
        aria-label="Collapse all categories"
      >
        Collapse All
      </button>
    {/snippet}
  </PageToolbar>

  <div class="flex min-h-0 flex-1 flex-col gap-2 px-4 pt-2.5 pb-3">
    {#if !dismissedRequiredDeps && installableRequiredDeps.length > 0}
      <MissingDepsBanner
        deps={installableRequiredDeps}
        title={`${installableRequiredDeps.length} missing ${installableRequiredDeps.length === 1 ? 'required dependency' : 'required dependencies'} detected`}
        actionLabel="Install required"
        {batchInstalling}
        oninstall={() => installMissingDeps(false)}
        ondismiss={() => (dismissedRequiredDeps = true)}
      />
    {/if}

    {#if !dismissedOptionalDeps && installableOptionalDeps.length > 0}
      <MissingDepsBanner
        deps={installableOptionalDeps}
        title={`${installableOptionalDeps.length} missing ${installableOptionalDeps.length === 1 ? 'optional dependency' : 'optional dependencies'} detected`}
        actionLabel="Install optional"
        {batchInstalling}
        oninstall={() => installMissingDeps(true)}
        ondismiss={() => (dismissedOptionalDeps = true)}
      />
    {/if}

    {#if loading}
      <div class="flex flex-col gap-1.5">
        {#each { length: 6 } as _, i (i)}
          <div class="flex items-center gap-3 rounded-lg border px-3 py-2.5">
            <Skeleton class="h-10 w-10 shrink-0 rounded-md" />
            <div class="flex flex-1 flex-col gap-1.5">
              <Skeleton class="h-4 w-48" />
              <Skeleton class="h-3 w-32" />
            </div>
          </div>
        {/each}
      </div>
    {:else if filteredAddons.length === 0}
      <div
        class="border-border flex flex-1 items-center justify-center rounded-lg border border-dashed p-8"
      >
        <div class="flex flex-col items-center gap-3 text-center">
          {#if addonPath === ''}
            <div class="bg-muted flex h-16 w-16 items-center justify-center rounded-full">
              <FolderOpen size={28} class="text-muted-foreground" />
            </div>
            <h3 class="text-lg font-medium">No addon path configured</h3>
            <p class="text-muted-foreground max-w-sm text-sm">
              Scribe couldn't detect your ESO AddOns folder automatically. Go to Settings to
              configure the path manually.
            </p>
          {:else if searchValue}
            <div class="bg-muted flex h-16 w-16 items-center justify-center rounded-full">
              <Search size={28} class="text-muted-foreground" />
            </div>
            <h3 class="text-lg font-medium">No results</h3>
            <p class="text-muted-foreground max-w-sm text-sm">
              No addons match "{searchValue}". Try a different search term.
            </p>
          {:else}
            <div class="bg-muted flex h-16 w-16 items-center justify-center rounded-full">
              <Package size={28} class="text-muted-foreground" />
            </div>
            <h3 class="text-lg font-medium">No addons found</h3>
            <p class="text-muted-foreground max-w-sm text-sm">
              Your installed addons will appear here. Make sure your ESO AddOns folder is configured
              correctly in Settings.
            </p>
          {/if}
        </div>
      </div>
    {:else}
      <div
        bind:this={scrollEl}
        class="min-h-0 flex-1 overflow-y-auto"
        style="contain: strict;"
      >
        <div style="height: {$virtualizerStore.getTotalSize()}px; width: 100%; position: relative;">
          <div
            style="position: absolute; top: 0; left: 0; width: 100%; transform: translateY({$virtualizerStore.getVirtualItems()[0]?.start ?? 0}px);"
          >
            {#each $virtualizerStore.getVirtualItems() as virtualItem (virtualItem.key)}
              {@const row = flatRows[virtualItem.index]}
              {#if row}
                <div data-index={virtualItem.index}>
                  {#if row.type === 'header'}
                    <CategoryHeader
                      name={row.group.name}
                      iconUrl={row.group.iconUrl}
                      count={row.group.addons.length}
                      expanded={expandedCategories.has(row.group.id)}
                      ontoggle={() => toggleCategory(row.group.id)}
                    />
                  {:else}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div class="py-0.5 pl-2 pr-0" oncontextmenu={(e) => openInstalledContextMenu(e, row.addon)}>
                      <AddonCard
                        addon={row.addon}
                        updateAvailable={hasUpdate(row.addon)}
                        categoryIconUrl={getCategoryIconUrl(row.addon)}
                        isThumbnail={getIsThumbnail(row.addon)}
                        onclick={() => openDetail(row.addon)}
                      />
                    </div>
                  {/if}
                </div>
              {/if}
            {/each}
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<AddonDetail
  addon={selectedAddon}
  open={detailOpen}
  onclose={() => (detailOpen = false)}
  matched={selectedAddon ? (matchedMap.get(selectedAddon.folderName.toLowerCase()) ?? null) : null}
  category={selectedAddon
    ? (() => {
        const m = matchedMap.get(selectedAddon.folderName.toLowerCase());
        return m?.remote?.categoryId ? (categoryMap.get(m.remote.categoryId) ?? null) : null;
      })()
    : null}
  {depUIDMap}
  installedFolderNames={installedFolderNamesSet}
  onuninstalled={() => {
    detailOpen = false;
    void refreshInstalledState();
    void checkMissingDeps();
  }}
/>
