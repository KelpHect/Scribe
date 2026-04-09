<svelte:options runes />

<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { Dialog, Badge, Button, Separator } from '$lib/components/ui';
  import { ScreenshotLightbox } from '$lib/components/addon';
  import ArrowUpCircle from 'lucide-svelte/icons/arrow-up-circle';
  import Calendar from 'lucide-svelte/icons/calendar';
  import CheckCircle from 'lucide-svelte/icons/check-circle';
  import Cpu from 'lucide-svelte/icons/cpu';
  import Download from 'lucide-svelte/icons/download';
  import Eye from 'lucide-svelte/icons/eye';
  import ExternalLink from 'lucide-svelte/icons/external-link';
  import Heart from 'lucide-svelte/icons/heart';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import Package from 'lucide-svelte/icons/package';
  import Tag from 'lucide-svelte/icons/tag';
  import User from 'lucide-svelte/icons/user';
  import { openExternalURL } from '$lib/services/runtime-service';
  import { getRemoteStore, getDownloadStore } from '$lib/stores';
  import { formatCompact, parseAddonChangelog, parseAddonDescription } from '$lib/utils';
  import type { RemoteAddon, RemoteAddonDetails, Category } from '$lib/services/esoui-service';

  interface Props {
    addon: RemoteAddon | null;
    open: boolean;
    onclose: () => void;
    installedFolderName?: string | null;
    category?: Category | null;
    updateAvailable?: boolean;
    oninstalled?: () => void;
  }

  const {
    addon,
    open,
    onclose,
    installedFolderName = null,
    category = null,
    updateAvailable = false,
    oninstalled
  }: Props = $props();

  const remote = getRemoteStore();
  const downloads = getDownloadStore();
  let localInstallError = $state<string | null>(null);

  let lightboxIndex = $state<number | null>(null);

  const detailsQuery = createQuery(() => ({
    queryKey: ['addon-details', addon?.uid ?? ''],
    queryFn: async (): Promise<RemoteAddonDetails | null> => {
      if (!addon) return null;
      return remote.getDetails(addon.uid);
    },
    enabled: open && !!addon?.uid,
    staleTime: 5 * 60 * 1000
  }));

  const details = $derived(
    open && addon ? ((detailsQuery.data as RemoteAddonDetails | null) ?? null) : null
  );
  const detailsLoading = $derived(open && !!addon && detailsQuery.isLoading);
  const detailsError = $derived(
    open && addon && detailsQuery.error instanceof Error ? detailsQuery.error.message : null
  );

  $effect(() => {
    if (!open || !addon) {
      localInstallError = null;
      lightboxIndex = null;
      return;
    }
    localInstallError = null;
    lightboxIndex = null;
  });

  const isInstalling = $derived(remote.installing && remote.installingUID === addon?.uid);
  const isInstalled = $derived(!!installedFolderName);

  const downloadTask = $derived(addon ? downloads.getTask(addon.uid) : undefined);
  const isInQueue = $derived(
    downloadTask?.state === 'queued' ||
      downloadTask?.state === 'downloading' ||
      downloadTask?.state === 'extracting'
  );
  const isActionDisabled = $derived(isInstalling || isInQueue || remote.installing);
  const installButtonLabel = $derived.by(() => {
    if (!downloadTask) return isInstalling ? 'Starting...' : '';
    switch (downloadTask.state) {
      case 'queued':
        return 'Queued';
      case 'downloading':
        return downloadTask.percent > 0 ? `${Math.round(downloadTask.percent)}%` : 'Downloading...';
      case 'extracting':
        return 'Extracting...';
      default:
        return '';
    }
  });

  async function runInstall(fallbackError: string) {
    if (!addon) return;
    localInstallError = null;
    try {
      await remote.install(addon.uid);
      oninstalled?.();
    } catch (e) {
      localInstallError = e instanceof Error ? e.message : fallbackError;
    }
  }

  const screenshots = $derived.by(() => {
    if (!addon) return [];
    const thumbs = addon.uiIMGThumbs ?? [];
    const full = addon.uiIMGs ?? [];
    return thumbs.length > 0
      ? thumbs.map((thumb, i) => ({ thumb, full: full[i] ?? thumb }))
      : full.map((img) => ({ thumb: img, full: img }));
  });

  const latestCompat = $derived(
    addon && addon.compatabilities && addon.compatabilities.length > 0
      ? addon.compatabilities[addon.compatabilities.length - 1]
      : null
  );

  const primaryAction = $derived.by(() => {
    if (updateAvailable) {
      return {
        label: 'Update',
        pendingLabel: installButtonLabel || 'Updating...',
        icon: ArrowUpCircle,
        onclick: () => runInstall('Update failed')
      };
    }
    if (isInstalled) {
      return null;
    }
    return {
      label: 'Install',
      pendingLabel: installButtonLabel || 'Installing...',
      icon: Download,
      onclick: () => runInstall('Install failed')
    };
  });

  const parsedDescription = $derived(
    details?.uiDescription
      ? parseAddonDescription(details.uiDescription)
      : { html: '', requiredLibraries: [], optionalLibraries: [] }
  );
  const descriptionHtml = $derived(parsedDescription.html);
  const requiredLibraries = $derived(parsedDescription.requiredLibraries);
  const optionalLibraries = $derived(parsedDescription.optionalLibraries);
  const changelogSections = $derived(details?.uiChangeLog ? parseAddonChangelog(details.uiChangeLog) : []);

  async function handleRichTextLinkClick(e: MouseEvent) {
    const target = e.target as HTMLElement | null;
    const anchor = target?.closest('a[href]') as HTMLAnchorElement | null;
    const href = anchor?.href;
    if (!href) return;
    e.preventDefault();
    await openExternalURL(href);
  }
</script>

<Dialog {open} {onclose} title={addon?.uiName ?? 'Addon Details'} panelClass="max-w-5xl">
  {#if !addon}
    <p class="text-muted-foreground text-sm">No addon selected.</p>
  {:else}
    <div class="flex flex-col gap-5">
      <div class="bg-muted/35 border-border flex flex-col gap-4 rounded-xl border p-4 md:flex-row md:items-start md:justify-between">
        <div class="min-w-0 flex-1">
          <div class="mb-2 flex flex-wrap items-center gap-2">
            {#if category?.name}
              <span class="bg-background/80 text-muted-foreground inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium">
                {#if category?.iconUrl}
                  <img
                    src={category.iconUrl}
                    alt=""
                    aria-hidden="true"
                    class="h-3.5 w-3.5 object-contain"
                    loading="lazy"
                  />
                {/if}
                {category.name}
              </span>
            {/if}
            {#if addon.uiVersion}
              <span class="bg-background/80 text-muted-foreground rounded-full border px-2.5 py-1 font-mono text-[11px]">v{addon.uiVersion}</span>
            {/if}
            {#if isInstalled && !updateAvailable}
              <Badge variant="outline" class="text-xs">
                <CheckCircle size={9} class="mr-1" />Installed
              </Badge>
            {/if}
            {#if updateAvailable}
              <Badge variant="destructive" class="text-xs">Update Available</Badge>
            {/if}
          </div>

          <div class="flex items-start gap-3">
            <div class="bg-background/80 hidden h-12 w-12 shrink-0 items-center justify-center rounded-lg border md:flex">
              {#if category?.iconUrl}
                <img
                  src={category.iconUrl}
                  alt=""
                  aria-hidden="true"
                  class="h-7 w-7 object-contain"
                  loading="lazy"
                />
              {:else}
                <Package size={20} class="text-muted-foreground" />
              {/if}
            </div>
            <div class="min-w-0 flex-1">
              <h3 class="text-foreground text-xl font-semibold leading-tight">{addon.uiName}</h3>
              <p class="text-muted-foreground mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
                <span class="inline-flex items-center gap-1.5"><User size={13} />{addon.uiAuthorName || 'Unknown'}</span>
                {#if latestCompat}
                  <span class="inline-flex items-center gap-1.5"><Cpu size={13} />{latestCompat.name} {latestCompat.version}</span>
                {/if}
                {#if addon.uiDate}
                  <span class="inline-flex items-center gap-1.5"><Calendar size={13} />Updated {addon.uiDate}</span>
                {/if}
              </p>
              <div class="text-muted-foreground mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-xs">
                <span class="inline-flex items-center gap-1.5"><Download size={12} class="text-primary" />{formatCompact(addon.uiDownloadTotal)} downloads</span>
                <span class="inline-flex items-center gap-1.5"><Heart size={12} class="text-primary" />{formatCompact(addon.uiFavoriteTotal)} favorites</span>
                <span class="inline-flex items-center gap-1.5"><Eye size={12} class="text-primary" />{details && details.uiHitCount > 0 ? formatCompact(details.uiHitCount) : 'n/a'} views</span>
              </div>
            </div>
          </div>
        </div>

        <div class="flex shrink-0 flex-col items-stretch gap-2 md:min-w-44 md:items-end">
          {#if primaryAction}
            <Button
              variant="default"
              size="sm"
              onclick={primaryAction.onclick}
              disabled={isActionDisabled}
            >
              {#if isActionDisabled}
                <Loader2 size={13} class="animate-spin" />{primaryAction.pendingLabel}
              {:else}
                <primaryAction.icon size={13} />{primaryAction.label}
              {/if}
            </Button>
          {:else}
            <Button variant="outline" size="sm" disabled><CheckCircle size={13} />Installed</Button>
          {/if}
          <div class="flex items-center gap-2 md:justify-end">
            {#if addon.uiFileInfoUrl}
              <a
                href={addon.uiFileInfoUrl}
                target="_blank"
                rel="noopener noreferrer"
                onclick={async (e) => { e.preventDefault(); await openExternalURL(addon.uiFileInfoUrl!); }}
                class="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 text-xs transition-colors"
              >
                <ExternalLink size={10} />ESOUI page
              </a>
            {/if}
          </div>
        </div>
      </div>

      {#if localInstallError || downloadTask?.error}
        <div class="border-destructive/50 bg-destructive/10 rounded-md p-3">
          <p class="text-destructive text-xs">{localInstallError || downloadTask?.error}</p>
        </div>
      {/if}

      {#if screenshots.length > 0}
        <div class="screenshot-rail -mx-1 flex gap-3 overflow-x-auto px-1 pb-2">
          {#each screenshots as shot, i (i)}
            <button
              type="button"
              class="shrink-0 cursor-zoom-in overflow-hidden rounded-lg border border-transparent text-left transition-all hover:-translate-y-0.5 hover:border-[var(--color-border)]"
              onclick={() => (lightboxIndex = i)}
              aria-label="View screenshot {i + 1}"
            >
              <img
                src={shot.thumb}
                alt="Screenshot {i + 1}"
                class="h-36 w-auto max-w-none rounded-lg object-cover transition-opacity hover:opacity-90"
                loading="lazy"
              />
            </button>
          {/each}
        </div>
      {:else}
        <div class="bg-muted/35 border-border flex h-28 w-full items-center justify-center rounded-xl border">
          {#if category?.iconUrl}
            <img
              src={category.iconUrl}
              alt=""
              aria-hidden="true"
              class="h-10 w-10 object-contain opacity-30"
              loading="lazy"
            />
          {:else}
            <Package size={32} class="text-muted-foreground opacity-30" />
          {/if}
        </div>
      {/if}

      <Separator />

      {#if detailsLoading}
        <div class="flex items-center gap-2 py-4">
          <Loader2 size={16} class="text-muted-foreground animate-spin" />
          <span class="text-muted-foreground text-sm">Loading details...</span>
        </div>
      {:else if detailsError}
        <p class="text-destructive text-sm">{detailsError}</p>
      {:else if details}
        {#if descriptionHtml}
          <div>
            <p class="text-foreground mb-2 border-b pb-1 text-sm font-semibold">Description</p>
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="bbcode-content text-foreground text-sm leading-relaxed"
              onclick={handleRichTextLinkClick}
            >
              <!-- eslint-disable-next-line svelte/no-at-html-tags -->
              {@html descriptionHtml}
            </div>
          </div>
        {/if}
        {#if requiredLibraries.length > 0 || optionalLibraries.length > 0}
          <Separator />
          <div class="bg-muted/20 border-border rounded-xl border p-4">
            <p class="text-foreground mb-3 text-sm font-semibold">Libraries</p>

            {#if requiredLibraries.length > 0}
              <div>
                <p class="text-foreground text-xs font-semibold uppercase tracking-wide">Required</p>
                <div class="mt-2 flex flex-wrap gap-2">
                  {#each requiredLibraries as lib (lib.name)}
                    {#if lib.url}
                      <a
                        href={lib.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        onclick={async (e) => {
                          e.preventDefault();
                          await openExternalURL(lib.url!);
                        }}
                        class="bg-background hover:bg-accent inline-flex items-center rounded-full border px-3 py-1.5 text-xs font-medium transition-colors"
                      >{lib.name}</a>
                    {:else}
                      <span class="bg-background inline-flex items-center rounded-full border px-3 py-1.5 text-xs font-medium">{lib.name}</span>
                    {/if}
                  {/each}
                </div>
              </div>
            {/if}

            {#if optionalLibraries.length > 0}
              <div class={requiredLibraries.length > 0 ? 'mt-4' : ''}>
                <p class="text-foreground text-xs font-semibold uppercase tracking-wide">Optional</p>
                <div class="mt-2 flex flex-wrap gap-2">
                  {#each optionalLibraries as lib (lib.name)}
                    {#if lib.url}
                      <a
                        href={lib.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        onclick={async (e) => {
                          e.preventDefault();
                          await openExternalURL(lib.url!);
                        }}
                        class="bg-background hover:bg-accent inline-flex items-center rounded-full border px-3 py-1.5 text-xs font-medium transition-colors"
                      >{lib.name}</a>
                    {:else}
                      <span class="bg-background inline-flex items-center rounded-full border px-3 py-1.5 text-xs font-medium">{lib.name}</span>
                    {/if}
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        {/if}
        {#if changelogSections.length > 0}
          <Separator />
          <details class="bg-muted/20 border-border rounded-xl border p-4">
            <summary class="text-foreground cursor-pointer list-none text-sm font-semibold">
              <div class="flex items-center justify-between gap-3">
                <span>Changelog</span>
                <span class="text-muted-foreground text-xs">{changelogSections.length} version{changelogSections.length === 1 ? '' : 's'}</span>
              </div>
            </summary>
            <div class="mt-3 flex flex-col gap-2 border-t pt-3">
              {#each changelogSections as section, index (section.title + index)}
                <details class="bg-background/70 border-border rounded-lg border px-3 py-2">
                  <summary class="text-foreground cursor-pointer list-none text-sm font-medium">
                    <div class="flex items-center justify-between gap-3">
                      <span>{section.title}</span>
                      <span class="text-muted-foreground text-[11px]">Expand</span>
                    </div>
                  </summary>
                  <div class="mt-3 border-t pt-3">
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div
                      class="bbcode-content text-foreground text-sm leading-relaxed"
                      onclick={handleRichTextLinkClick}
                    >
                      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                      {@html section.html}
                    </div>
                  </div>
                </details>
              {/each}
            </div>
          </details>
        {/if}
        {#if !descriptionHtml && changelogSections.length === 0}
          <p class="text-muted-foreground text-sm">No additional details available.</p>
        {/if}
      {:else}
        <p class="text-muted-foreground text-sm">No additional details available.</p>
      {/if}

      {#if isInstalled}
        <Separator />
        <p class="text-muted-foreground text-xs">
          Dependency information is available on the Installed tab after installation.
        </p>
      {/if}
    </div>
  {/if}
</Dialog>

{#if lightboxIndex !== null}
  <ScreenshotLightbox
    {screenshots}
    index={lightboxIndex}
    onclose={() => (lightboxIndex = null)}
    onprev={() => {
      const current = lightboxIndex;
      if (current !== null && screenshots.length > 0)
        lightboxIndex = (current - 1 + screenshots.length) % screenshots.length;
    }}
    onnext={() => {
      const current = lightboxIndex;
      if (current !== null && screenshots.length > 0)
        lightboxIndex = (current + 1) % screenshots.length;
    }}
  />
{/if}

<style>
  .screenshot-rail {
    scrollbar-width: thin;
    scrollbar-color: var(--color-border) transparent;
  }

  .screenshot-rail::-webkit-scrollbar {
    height: 10px;
  }

  .screenshot-rail::-webkit-scrollbar-track {
    background: transparent;
  }

  .screenshot-rail::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--color-border) 85%, transparent);
    border-radius: 999px;
  }

  .screenshot-rail::-webkit-scrollbar-thumb:hover {
    background: color-mix(in srgb, var(--color-foreground) 20%, var(--color-border));
  }
</style>
