<svelte:options runes />

<script lang="ts">
  import { onMount } from 'svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import CheckCircle2 from 'lucide-svelte/icons/check-circle-2';
  import FolderInput from 'lucide-svelte/icons/folder-input';
  import FolderOpen from 'lucide-svelte/icons/folder-open';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import RefreshCw from 'lucide-svelte/icons/refresh-cw';
  import Save from 'lucide-svelte/icons/save';
  import Settings from 'lucide-svelte/icons/settings';
  import Wand2 from 'lucide-svelte/icons/wand-2';
  import { Button } from '$lib/components/ui';
  import { PageToolbar } from '$lib/components/layout';
  import {
    fetchDiagnostics,
    type DiagnosticsSnapshot
  } from '$lib/services/diagnostics-service';
  import { fetchAppInfo, type AppInfo } from '$lib/services/app-info-service';
  import {
    browseForFolder,
    fetchInstalledAddons,
    fetchAddonPath,
    fetchDetectedPath,
    updateAddonPath,
    type Addon
  } from '$lib/services/addon-service';
  import { getSettings, saveSettings } from '$lib/services/settings-service';
  import { applyTheme, type AppTheme } from '$lib/services/theme-service';
  import { openExternalURL } from '$lib/services/runtime-service';
  import {
    addonPathQueryKey,
    installedAddonsQueryKey,
    refreshInstalledState
  } from '$lib/db/query-state';
  import { queryClient } from '$lib/db/client';
  import { toast } from 'svelte-sonner';
  import { createForm } from '@tanstack/svelte-form';
  import * as v from 'valibot';

  type FrontendDiagnostics = {
    addonDetailQueries: number;
    addonDetailQueriesWithData: number;
    addonDetailFresh: number;
    addonDetailStale: number;
    cachedUIDs: string[];
  };

  let detectedPath = $state('');
  let isBrowsing = $state(false);
  let isApplyingDetected = $state(false);
  let loadingSettings = $state(false);
  let diagnostics = $state<DiagnosticsSnapshot | null>(null);
  let diagnosticsLoading = $state(false);
  let appInfo = $state<AppInfo | null>(null);
  let showShortcuts = $state(false);
  let showLibraries = $state(false);
  let frontendDiagnostics = $state<FrontendDiagnostics>({
    addonDetailQueries: 0,
    addonDetailQueriesWithData: 0,
    addonDetailFresh: 0,
    addonDetailStale: 0,
    cachedUIDs: []
  });

  const showDiagnostics = import.meta.env.DEV;

  async function applyThemeSelection(theme: AppTheme) {
    form.setFieldValue('theme', theme);
    applyTheme(theme);

    try {
      await saveSettings({
        ...form.state.values,
        addonPath: form.state.values.addonPath,
        autoUpdate: form.state.values.autoUpdate,
        memoryLimitMb: form.state.values.memoryLimitMb,
        theme
      });
      window.dispatchEvent(
        new CustomEvent('scribe:settings-updated', {
          detail: { memoryLimitMb: form.state.values.memoryLimitMb, theme }
        })
      );
    } catch (e) {
      toast.error('Failed to apply theme', { description: String(e) });
    }
  }

  const installedQuery = createQuery(() => ({
    queryKey: installedAddonsQueryKey,
    queryFn: async (): Promise<Addon[]> => fetchInstalledAddons()
  }));
  const addonPathQuery = createQuery(() => ({
    queryKey: addonPathQueryKey,
    queryFn: async (): Promise<string> => fetchAddonPath()
  }));

  const addonPath = $derived((addonPathQuery.data as string) ?? '');
  const installedAddons = $derived((installedQuery.data as Addon[]) ?? []);

  function validateAddonPath(val: string) {
    const result = v.safeParse(v.pipe(v.string(), v.trim()), val);
    if (!result.success) return result.issues[0].message;
    return undefined;
  }

  const form = createForm(() => ({
    defaultValues: {
      addonPath: '',
      autoUpdate: false,
      memoryLimitMb: 150,
      theme: 'scribe' as AppTheme
    },
    onSubmit: async ({ value }) => {
      try {
        const pathChanged = value.addonPath !== addonPath;
        await saveSettings(value);
        applyTheme(value.theme);
        if (pathChanged) {
          await refreshInstalledState();
        }
        window.dispatchEvent(
          new CustomEvent('scribe:settings-updated', {
            detail: { memoryLimitMb: value.memoryLimitMb, theme: value.theme }
          })
        );
        toast.success('Settings saved');
      } catch (e) {
        toast.error('Failed to save settings', { description: String(e) });
      }
    }
  }));

  onMount(async () => {
    loadingSettings = true;
    try {
      const [settings, detected, currentPath, info] = await Promise.all([
        getSettings(),
        fetchDetectedPath(),
        fetchAddonPath(),
        fetchAppInfo().catch(() => null)
      ]);
      detectedPath = detected;
      appInfo = info;
      form.setFieldValue('addonPath', settings.addonPath || currentPath || '');
      form.setFieldValue('autoUpdate', settings.autoUpdate);
      form.setFieldValue('memoryLimitMb', settings.memoryLimitMb ?? 150);
      form.setFieldValue('theme', settings.theme);
    } catch {
      detectedPath = '';
    } finally {
      loadingSettings = false;
    }

    if (showDiagnostics) {
      void loadDiagnostics();
    }
  });

  function collectFrontendDiagnostics(): FrontendDiagnostics {
    const queries = queryClient.getQueryCache().findAll({ queryKey: ['addon-details'] });
    const now = Date.now();
    const staleThresholdMs = 5 * 60 * 1000;

    let withData = 0;
    let fresh = 0;
    let stale = 0;
    const cachedUIDs: string[] = [];

    for (const query of queries) {
      const hasData = query.state.data !== undefined && query.state.data !== null;
      if (!hasData) continue;

      withData++;
      const age = now - query.state.dataUpdatedAt;
      if (age <= staleThresholdMs) {
        fresh++;
      } else {
        stale++;
      }

      const uid = typeof query.queryKey[1] === 'string' ? query.queryKey[1] : null;
      if (uid) cachedUIDs.push(uid);
    }

    return {
      addonDetailQueries: queries.length,
      addonDetailQueriesWithData: withData,
      addonDetailFresh: fresh,
      addonDetailStale: stale,
      cachedUIDs: cachedUIDs.slice(0, 8)
    };
  }

  async function loadDiagnostics() {
    if (!showDiagnostics) return;
    diagnosticsLoading = true;
    try {
      diagnostics = await fetchDiagnostics();
      frontendDiagnostics = collectFrontendDiagnostics();
    } finally {
      diagnosticsLoading = false;
    }
  }

  async function useDetectedPath() {
    if (!detectedPath) return;
    isApplyingDetected = true;
    try {
      form.setFieldValue('addonPath', detectedPath);
      await updateAddonPath(detectedPath);
      await refreshInstalledState();
      toast.success('AddOns folder updated', { description: detectedPath });
    } catch (e) {
      toast.error('Failed to set detected path', { description: String(e) });
    } finally {
      isApplyingDetected = false;
    }
  }

  async function browseAddonPath() {
    isBrowsing = true;
    try {
      const path = await browseForFolder('Select ESO AddOns folder');
      if (path) {
        form.setFieldValue('addonPath', path);
      }
    } catch (e) {
      toast.error('Failed to browse folder', { description: String(e) });
    } finally {
      isBrowsing = false;
    }
  }

  async function rescan() {
    await refreshInstalledState();
    if (installedAddons.length > 0) {
      toast.success('Scan complete', { description: `Found ${installedAddons.length} addons` });
    } else {
      toast.info('Scan complete', { description: 'No addons found' });
    }
  }
</script>

<div class="flex h-full flex-col">
  <PageToolbar title="Settings" subtitle="Configure Scribe paths and preferences">
    {#snippet icon()}
      <Settings size={14} class="text-[var(--color-toolbar-foreground)]" />
    {/snippet}
  </PageToolbar>

  <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-4 pt-3 pb-4">
    {#if loadingSettings}
      <div class="text-muted-foreground flex items-center gap-2 text-sm">
        <Loader2 size={14} class="animate-spin" />
        Loading settings...
      </div>
    {:else}
      <form
        onsubmit={(e) => {
          e.preventDefault();
          e.stopPropagation();
          void form.handleSubmit();
        }}
        class="flex flex-col gap-4"
      >
        <div class="card-elevated bg-card border-border rounded-lg border p-4">
          <h3 class="text-sm font-medium">AddOns Folder</h3>
          <p class="text-muted-foreground mt-1 text-xs">
            The directory where ESO stores your installed addons.
          </p>

          <form.Field
            name="addonPath"
            validators={{ onChange: ({ value }) => validateAddonPath(value) }}
          >
            {#snippet children(field)}
              <div class="mt-3 flex flex-col gap-2">
                <div
                  class="text-muted-foreground bg-muted flex items-center gap-2 rounded-md px-3 py-2 font-mono text-xs"
                >
                  <FolderOpen size={14} class="shrink-0" />
                  <span class="truncate">{field.state.value || 'Not configured'}</span>
                </div>

                {#if detectedPath}
                  <div class="flex items-center gap-2">
                    <div
                      class="text-muted-foreground flex min-w-0 flex-1 items-center gap-2 text-xs"
                    >
                      <CheckCircle2 size={12} class="text-success shrink-0" />
                      <span class="truncate"
                        >Detected: <span class="font-mono">{detectedPath}</span></span
                      >
                    </div>
                    <button
                      type="button"
                      onclick={useDetectedPath}
                      disabled={isApplyingDetected || field.state.value === detectedPath}
                      class="border-border hover:bg-accent inline-flex shrink-0 cursor-pointer items-center gap-1.5 rounded-md border px-2.5 py-1 text-xs font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      <Wand2 size={12} />
                      Use this path
                    </button>
                  </div>
                {/if}

                <div class="mt-1 flex gap-2">
                  <button
                    type="button"
                    onclick={browseAddonPath}
                    disabled={isBrowsing}
                    class="border-border bg-primary text-primary-foreground hover:bg-primary/90 inline-flex cursor-pointer items-center gap-2 rounded-md px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50"
                  >
                    <FolderInput size={14} />
                    Browse...
                  </button>
                  <button
                    type="button"
                    onclick={rescan}
                    class="border-border hover:bg-accent inline-flex cursor-pointer items-center gap-2 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors"
                  >
                    <RefreshCw size={14} />
                    Rescan
                  </button>
                </div>
                {#if field.state.meta.errors.length > 0}
                  <p class="text-destructive text-xs">{field.state.meta.errors[0]}</p>
                {/if}
              </div>
            {/snippet}
          </form.Field>
        </div>

        <div class="card-elevated bg-card border-border rounded-lg border p-4">
          <h3 class="mb-3 text-sm font-medium">Preferences</h3>
          <div class="flex flex-col gap-4">
            <form.Field name="autoUpdate">
              {#snippet children(field)}
                <div class="flex items-center justify-between">
                  <div>
                    <p class="text-sm font-medium">Auto Update</p>
                    <p class="text-muted-foreground text-xs">
                      Automatically update addons when updates are available.
                    </p>
                  </div>
                  <button
                    type="button"
                    role="switch"
                    aria-label="Toggle auto update"
                    aria-checked={field.state.value}
                    onclick={() => field.handleChange(!field.state.value)}
                    class={[
                      'relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:outline-none',
                      field.state.value ? 'bg-primary' : 'bg-input'
                    ].join(' ')}
                  >
                    <span
                      class={[
                        'bg-background pointer-events-none block h-4 w-4 rounded-full shadow-lg ring-0 transition-transform',
                        field.state.value ? 'translate-x-4' : 'translate-x-0'
                      ].join(' ')}
                    ></span>
                  </button>
                </div>
              {/snippet}
            </form.Field>

            <form.Field name="theme">
              {#snippet children(field)}
                <div class="flex items-start justify-between gap-4">
                  <div>
                    <p class="text-sm font-medium">Theme</p>
                    <p class="text-muted-foreground text-xs">
                      Pick the app palette. Scribe keeps the warm default, Neutral keeps the content light with dark chrome, and Dark stays fully dark.
                    </p>
                  </div>
                  <div class="grid shrink-0 gap-2 sm:grid-cols-3">
                    {#each [
                      { value: 'scribe', label: 'Scribe', swatch: 'from-amber-800 via-stone-800 to-amber-200' },
                      { value: 'neutral', label: 'Neutral', swatch: 'from-zinc-900 via-zinc-500 to-white' },
                      { value: 'dark', label: 'Dark', swatch: 'from-zinc-950 via-zinc-700 to-zinc-400' }
                    ] as option (option.value)}
                      <button
                        type="button"
                        onclick={() => applyThemeSelection(option.value as AppTheme)}
                        class={[
                          'border-border bg-background hover:bg-accent flex min-w-24 flex-col items-start gap-2 rounded-lg border px-3 py-2 text-left transition-colors',
                          field.state.value === option.value ? 'ring-2 ring-[var(--color-ring)]' : ''
                        ].join(' ')}
                      >
                        <span class={`h-3 w-full rounded-full bg-linear-to-r ${option.swatch}`}></span>
                        <span class="text-sm font-medium">{option.label}</span>
                      </button>
                    {/each}
                  </div>
                </div>
              {/snippet}
            </form.Field>

            <form.Field name="memoryLimitMb">
              {#snippet children(field)}
                <div class="flex items-start justify-between gap-4">
                  <div>
                    <p class="text-sm font-medium">Memory Cleanup Threshold</p>
                    <p class="text-muted-foreground text-xs">
                      When Go runtime memory reaches this many MB, Scribe clears detail caches and
                      forces a memory cleanup. Use 0 to disable automatic cleanup.
                    </p>
                  </div>
                  <div class="flex shrink-0 items-center gap-2">
                    <button
                      type="button"
                      onclick={() => field.handleChange(100)}
                      class="border-border hover:bg-accent rounded-md border px-2 py-1 text-xs font-medium"
                    >
                      100 MB
                    </button>
                    <button
                      type="button"
                      onclick={() => field.handleChange(150)}
                      class="border-border hover:bg-accent rounded-md border px-2 py-1 text-xs font-medium"
                    >
                      150 MB
                    </button>
                    <input
                      type="number"
                      min="0"
                      step="10"
                      value={field.state.value}
                      oninput={(e) => field.handleChange(Number((e.target as HTMLInputElement).value || 0))}
                      class="border-border bg-background w-24 rounded-md border px-2 py-1 text-right text-sm"
                    />
                    <span class="text-muted-foreground text-xs">MB</span>
                  </div>
                </div>
              {/snippet}
            </form.Field>
          </div>
        </div>

        {#if showDiagnostics}
          <div class="card-elevated bg-card border-border rounded-lg border p-4">
            <div class="mb-3 flex items-center justify-between gap-3">
              <div>
                <h3 class="text-sm font-medium">Diagnostics</h3>
                <p class="text-muted-foreground mt-1 text-xs">
                  Dev-only runtime metrics for startup, memory, and detail-query caching.
                </p>
              </div>
              <button
                type="button"
                onclick={loadDiagnostics}
                disabled={diagnosticsLoading}
                class="border-border hover:bg-accent inline-flex cursor-pointer items-center gap-2 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50"
              >
                <RefreshCw size={14} class={diagnosticsLoading ? 'animate-spin' : ''} />
                Refresh
              </button>
            </div>

            {#if diagnostics}
              <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                <div class="bg-muted/40 rounded-md border p-3">
                  <p class="mb-2 text-xs font-medium">
                    Startup
                    <span class={diagnostics.startupBudgetOk ? 'text-success' : 'text-destructive'}>
                      {diagnostics.startupBudgetOk ? '✓ within budget' : '⚠ over budget'}
                    </span>
                  </p>
                  <div class="text-muted-foreground space-y-1 text-xs">
                    <p>Startup: <span class="text-foreground font-mono">{diagnostics.startupMs} ms</span></p>
                    <p>DOM ready: <span class="text-foreground font-mono">{diagnostics.domReadyMs} ms</span></p>
                    <p>Frontend ready: <span class="text-foreground font-mono">{diagnostics.frontendReadyMs} ms</span> <span class="text-muted-foreground">(target &lt;1s)</span></p>
                    <p>Remote ready: <span class="text-foreground font-mono">{diagnostics.remoteReadyMs} ms</span></p>
                  </div>
                </div>

                <div class="bg-muted/40 rounded-md border p-3">
                  <p class="mb-2 text-xs font-medium">
                    Memory
                    <span class={diagnostics.memoryBudgetOk ? 'text-success' : 'text-destructive'}>
                      {diagnostics.memoryBudgetOk ? '✓ within budget' : '⚠ over budget'}
                    </span>
                  </p>
                  <div class="text-muted-foreground space-y-1 text-xs">
                    <p>Heap alloc: <span class="text-foreground font-mono">{diagnostics.heapAllocMb} MB</span></p>
                    <p>Heap in-use: <span class="text-foreground font-mono">{diagnostics.heapInUseMb} MB</span></p>
                    <p>Sys: <span class="text-foreground font-mono">{diagnostics.sysMb} MB</span> <span class="text-muted-foreground">(target &lt;150MB)</span></p>
                    <p>Stack in-use: <span class="text-foreground font-mono">{diagnostics.stackInUseMb} MB</span></p>
                    <p>Total alloc: <span class="text-foreground font-mono">{diagnostics.totalAllocMb} MB</span></p>
                    <p>Goroutines: <span class="text-foreground font-mono">{diagnostics.goroutines}</span></p>
                    <p>GC runs: <span class="text-foreground font-mono">{diagnostics.numGc}</span></p>
                  </div>
                </div>

                <div class="bg-muted/40 rounded-md border p-3">
                  <p class="mb-2 text-xs font-medium">Dataset</p>
                  <div class="text-muted-foreground space-y-1 text-xs">
                    <p>Remote addons: <span class="text-foreground font-mono">{diagnostics.remoteAddons}</span></p>
                    <p>Remote categories: <span class="text-foreground font-mono">{diagnostics.remoteCategories}</span></p>
                    <p>Installed addons: <span class="text-foreground font-mono">{diagnostics.installedAddons}</span></p>
                    <p>Cache stale: <span class="text-foreground font-mono">{diagnostics.remoteCacheStale ? 'yes' : 'no'}</span></p>
                  </div>
                </div>

                <div class="bg-muted/40 rounded-md border p-3 md:col-span-2 xl:col-span-1">
                  <p class="mb-2 text-xs font-medium">Detail Fetches</p>
                  <div class="text-muted-foreground space-y-1 text-xs">
                    <p>Total backend calls: <span class="text-foreground font-mono">{diagnostics.detailRequests}</span></p>
                    <p>Unique UIDs: <span class="text-foreground font-mono">{diagnostics.detailUniqueUids}</span></p>
                    <p>Last UID: <span class="text-foreground font-mono">{diagnostics.lastDetailUid || 'n/a'}</span></p>
                    <p>Last at: <span class="text-foreground font-mono">{diagnostics.lastDetailAt || 'n/a'}</span></p>
                  </div>
                  {#if diagnostics.detailTop.length > 0}
                    <div class="mt-3 space-y-1 text-xs">
                      {#each diagnostics.detailTop as stat (stat.name)}
                        <p class="text-muted-foreground flex items-center justify-between gap-2">
                          <span class="truncate font-mono">{stat.name}</span>
                          <span class="text-foreground shrink-0 font-mono">{stat.count}</span>
                        </p>
                      {/each}
                    </div>
                  {/if}
                </div>

                <div class="bg-muted/40 rounded-md border p-3 md:col-span-2 xl:col-span-2">
                  <p class="mb-2 text-xs font-medium">Frontend Detail Query Cache</p>
                  <div class="text-muted-foreground grid gap-1 text-xs md:grid-cols-2">
                    <p>Total queries: <span class="text-foreground font-mono">{frontendDiagnostics.addonDetailQueries}</span></p>
                    <p>With data: <span class="text-foreground font-mono">{frontendDiagnostics.addonDetailQueriesWithData}</span></p>
                    <p>Fresh (5m): <span class="text-foreground font-mono">{frontendDiagnostics.addonDetailFresh}</span></p>
                    <p>Stale: <span class="text-foreground font-mono">{frontendDiagnostics.addonDetailStale}</span></p>
                    <p>Remote refreshes: <span class="text-foreground font-mono">{diagnostics.remoteRefreshCount}</span></p>
                    <p>Last refresh: <span class="text-foreground font-mono">{diagnostics.lastRemoteRefreshAt || 'n/a'}</span></p>
                    <p class="md:col-span-2">Last refresh duration: <span class="text-foreground font-mono">{diagnostics.lastRemoteRefreshMs} ms</span></p>
                  </div>
                  {#if frontendDiagnostics.cachedUIDs.length > 0}
                    <div class="mt-3 flex flex-wrap gap-1.5">
                      {#each frontendDiagnostics.cachedUIDs as uid (uid)}
                        <span class="rounded-md border px-2 py-0.5 font-mono text-[11px]">{uid}</span>
                      {/each}
                    </div>
                  {/if}
                </div>
              </div>
            {:else if diagnosticsLoading}
              <div class="text-muted-foreground flex items-center gap-2 text-sm">
                <Loader2 size={14} class="animate-spin" />
                Loading diagnostics...
              </div>
            {:else}
              <p class="text-muted-foreground text-sm">Diagnostics not loaded yet.</p>
            {/if}
          </div>
        {/if}

        <form.Subscribe>
          {#snippet children(state)}
            <div class="flex gap-2">
              <Button type="submit" disabled={state.isSubmitting}>
                {#if state.isSubmitting}
                  <Loader2 size={15} class="animate-spin" />
                  Saving...
                {:else}
                  <Save size={15} />
                  Save Settings
                {/if}
              </Button>
              {#if state.isDirty}
                <Button type="button" variant="ghost" onclick={() => form.reset()}>
                  Discard changes
                </Button>
              {/if}
            </div>
          {/snippet}
        </form.Subscribe>

        <div class="card-elevated bg-card border-border rounded-lg border p-4">
          <div class="flex items-center justify-between">
            <h3 class="text-sm font-medium">Keyboard Shortcuts</h3>
            <button
              type="button"
              onclick={() => (showShortcuts = !showShortcuts)}
              class="text-muted-foreground hover:text-foreground text-xs transition-colors"
            >
              {showShortcuts ? 'Hide' : 'Show all'}
            </button>
          </div>
          {#if showShortcuts}
            <div class="mt-3 grid gap-2 text-xs md:grid-cols-2">
              <div class="flex items-center justify-between gap-4">
                <span class="text-muted-foreground">Installed Addons</span>
                <kbd class="bg-muted rounded border px-1.5 py-0.5 font-mono">Ctrl+1</kbd>
              </div>
              <div class="flex items-center justify-between gap-4">
                <span class="text-muted-foreground">Find More</span>
                <kbd class="bg-muted rounded border px-1.5 py-0.5 font-mono">Ctrl+2</kbd>
              </div>
              <div class="flex items-center justify-between gap-4">
                <span class="text-muted-foreground">Updates</span>
                <kbd class="bg-muted rounded border px-1.5 py-0.5 font-mono">Ctrl+U</kbd>
              </div>
              <div class="flex items-center justify-between gap-4">
                <span class="text-muted-foreground">Focus Search</span>
                <kbd class="bg-muted rounded border px-1.5 py-0.5 font-mono">Ctrl+F</kbd>
              </div>
              <div class="flex items-center justify-between gap-4">
                <span class="text-muted-foreground">Close Modal / Menu</span>
                <kbd class="bg-muted rounded border px-1.5 py-0.5 font-mono">Esc</kbd>
              </div>
              <div class="flex items-center justify-between gap-4">
                <span class="text-muted-foreground">Right-click</span>
                <span class="text-muted-foreground">Context menu</span>
              </div>
            </div>
          {/if}
        </div>

        {#if appInfo}
          <div class="card-elevated bg-card border-border rounded-lg border p-4">
            <h3 class="text-sm font-medium">About Scribe</h3>
            <div class="text-muted-foreground mt-2 space-y-1 text-xs">
              <p>Version: <span class="text-foreground font-mono">{appInfo.version}</span>
                {#if appInfo.commit !== 'none'}
                  (<span class="font-mono">{appInfo.commit}</span>)
                {/if}
              </p>
              {#if appInfo.buildDate && appInfo.buildDate !== 'unknown'}
                <p>Built: <span class="text-foreground font-mono">{appInfo.buildDate}</span></p>
              {/if}
              <p>Runtime: <span class="text-foreground font-mono">{appInfo.goVersion}</span> · <span class="font-mono">{appInfo.os}/{appInfo.arch}</span></p>
            </div>
          </div>
        {/if}

        <div class="card-elevated bg-card border-border rounded-lg border p-4">
          <h3 class="text-sm font-medium">Credits &amp; Attribution</h3>

          <div class="mt-3">
            <p class="text-xs font-medium">Addon Data</p>
            <p class="text-muted-foreground mt-1 text-xs leading-relaxed">
              Addon listings, metadata, thumbnails, and downloads are provided by
              <button
                type="button"
                onclick={() => void openExternalURL('https://www.esoui.com')}
                class="text-primary cursor-pointer underline-offset-2 hover:underline"
              >ESOUI.com</button>
              via the public
              <button
                type="button"
                onclick={() => void openExternalURL('https://api.mmoui.com')}
                class="text-primary cursor-pointer underline-offset-2 hover:underline"
              >MMOUI API</button>.
              All addon content remains the property of its respective authors.
              Scribe does not redistribute or mirror any addon files.
            </p>
          </div>

          <div class="border-border mt-3 border-t pt-3">
            <p class="text-xs font-medium">Trademark Notice</p>
            <p class="text-muted-foreground mt-1 text-xs leading-relaxed">
              <em>The Elder Scrolls Online</em> is a registered trademark of ZeniMax Media Inc.
              Scribe is an independent community tool and is not affiliated with,
              endorsed by, or connected to ZeniMax Media Inc. or Bethesda Softworks LLC.
            </p>
          </div>

          <div class="border-border mt-3 border-t pt-3">
            <div class="flex items-center justify-between">
              <p class="text-xs font-medium">Open Source Libraries</p>
              <button
                type="button"
                onclick={() => (showLibraries = !showLibraries)}
                class="text-muted-foreground hover:text-foreground text-xs transition-colors"
              >
                {showLibraries ? 'Hide' : 'Show all'}
              </button>
            </div>
            {#if showLibraries}
              <div class="mt-3 flex flex-col gap-1.5">
                {#each [
                  { name: 'Wails',            license: 'MIT',       url: 'https://github.com/wailsapp/wails' },
                  { name: 'Svelte',           license: 'MIT',       url: 'https://github.com/sveltejs/svelte' },
                  { name: 'Tailwind CSS',     license: 'MIT',       url: 'https://github.com/tailwindlabs/tailwindcss' },
                  { name: 'TanStack Query',   license: 'MIT',       url: 'https://github.com/TanStack/query' },
                  { name: 'TanStack Form',    license: 'MIT',       url: 'https://github.com/TanStack/form' },
                  { name: 'TanStack Virtual', license: 'MIT',       url: 'https://github.com/TanStack/virtual' },
                  { name: 'Lucide',           license: 'ISC',       url: 'https://github.com/lucide-icons/lucide' },
                  { name: 'svelte-sonner',    license: 'MIT',       url: 'https://github.com/wobsoriano/svelte-sonner' },
                  { name: 'Valibot',          license: 'MIT',       url: 'https://github.com/fabian-hiller/valibot' },
                  { name: 'GORM',             license: 'MIT',       url: 'https://github.com/go-gorm/gorm' },
                  { name: 'glebarez/sqlite',  license: 'MIT',       url: 'https://github.com/glebarez/sqlite' },
                  { name: 'google/uuid',      license: 'BSD-3',     url: 'https://github.com/google/uuid' },
                  { name: 'go-toast',         license: 'MIT',       url: 'https://git.sr.ht/~jackmordaunt/go-toast' },
                  { name: 'Vite',             license: 'MIT',       url: 'https://github.com/vitejs/vite' },
                ] as lib (lib.name)}
                  <div class="flex items-center justify-between gap-2">
                    <button
                      type="button"
                      onclick={() => void openExternalURL(lib.url)}
                      class="text-primary cursor-pointer text-xs underline-offset-2 hover:underline"
                    >{lib.name}</button>
                    <span class="text-muted-foreground bg-muted rounded px-1.5 py-0.5 font-mono text-[10px]">{lib.license}</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>

       </form>
     {/if}
   </div>
 </div>
