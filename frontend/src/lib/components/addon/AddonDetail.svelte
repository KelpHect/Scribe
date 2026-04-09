<svelte:options runes />

<script lang="ts">
  import { Dialog, Badge, Button, Separator } from '$lib/components/ui';
  import {
    StatsPill,
    DependencyList,
    OptionalDependencyList,
    AddonMetadataGrid
  } from '$lib/components/addon';
  import ArrowUpCircle from 'lucide-svelte/icons/arrow-up-circle';
  import Library from 'lucide-svelte/icons/library';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import Package from 'lucide-svelte/icons/package';
  import Trash2 from 'lucide-svelte/icons/trash-2';
  import type { Addon } from '$lib/services/addon-service';
  import type { Category, MatchedAddon } from '$lib/services/esoui-service';
  import { getRemoteStore, navigation } from '$lib/stores';
  import { uninstallRemoteAddon } from '$lib/stores/remote-mutations.svelte';
  import { batchInstall } from '$lib/stores/downloads.svelte';

  interface Props {
    addon: Addon | null;
    open: boolean;
    onclose: () => void;
    matched?: MatchedAddon | null;
    category?: Category | null;
    depUIDMap?: Map<string, string>;
    installedFolderNames?: Set<string>;
    onuninstalled?: () => void;
  }

  const {
    addon,
    open,
    onclose,
    matched = null,
    category = null,
    depUIDMap = new Map(),
    installedFolderNames = new Set(),
    onuninstalled
  }: Props = $props();

  const remote = getRemoteStore();

  const iconUrl = $derived(matched?.remote?.uiIMGs?.[0] ?? category?.iconUrl ?? null);

  const isInstalling = $derived(
    remote.installing && remote.installingUID === (matched?.remote?.uid ?? null)
  );

  let installError = $state<string | null>(null);

  let uninstallConfirming = $state(false);
  let isUninstalling = $state(false);
  let uninstallError = $state<string | null>(null);

  let depInstallingUID = $state<string | null>(null);
  let depInstallErrors = $state<Record<string, string>>({});
  let batchInstallingState = $state(false);
  let batchInstallError = $state<string | null>(null);

  async function handleUpdate() {
    if (!matched?.remote) return;
    installError = null;
    try {
      await remote.install(matched.remote.uid);
    } catch (e: unknown) {
      installError = e instanceof Error ? e.message : 'Update failed';
    }
  }

  function requestUninstall() {
    uninstallError = null;
    uninstallConfirming = true;
  }

  function cancelUninstall() {
    uninstallConfirming = false;
  }

  async function confirmUninstall() {
    if (!addon) return;
    isUninstalling = true;
    uninstallError = null;
    try {
      await uninstallRemoteAddon(addon.folderName, addon.title);
      onuninstalled?.();
      onclose();
    } catch (e: unknown) {
      uninstallError = e instanceof Error ? e.message : 'Uninstall failed';
      uninstallConfirming = false;
    } finally {
      isUninstalling = false;
    }
  }

  function wouldCreateCycle(root: string, target: string): boolean {
    return target.replace(/[><=]+\d+.*$/, '').trim().toLowerCase() === root.toLowerCase();
  }

  const hasCycle = $derived(
    addon ? (addon.dependsOn ?? []).some((dep) => wouldCreateCycle(addon.folderName, dep)) : false
  );

  const missingInstallableUIDs = $derived(
    addon
      ? (addon.dependsOn ?? [])
          .filter((dep) => !installedFolderNames.has(dep.replace(/[><=]+\d+.*$/, '').trim().toLowerCase()))
          .map((dep) => depUIDMap.get(dep.replace(/[><=]+\d+.*$/, '').trim().toLowerCase()) ?? null)
          .filter((uid): uid is string => uid !== null)
      : []
  );

  async function installAllMissing() {
    if (missingInstallableUIDs.length === 0) return;
    batchInstallingState = true;
    batchInstallError = null;
    try {
      await batchInstall(missingInstallableUIDs);
    } catch (e: unknown) {
      batchInstallError = e instanceof Error ? e.message : 'Batch install failed';
    } finally {
      batchInstallingState = false;
    }
  }

  async function installDep(dep: string, uid: string) {
    depInstallingUID = uid;
    depInstallErrors = { ...depInstallErrors, [dep]: '' };
    try {
      await remote.install(uid);
    } catch (e: unknown) {
      depInstallErrors = {
        ...depInstallErrors,
        [dep]: e instanceof Error ? e.message : 'Install failed'
      };
    } finally {
      depInstallingUID = null;
    }
  }

  function searchDepInFindMore(dep: string) {
    const name = dep.replace(/[><=]+\d+.*$/, '').trim();
    onclose();
    navigation.navigate('find-more', name);
  }
</script>

<Dialog {open} {onclose} title={addon?.title ?? 'Addon Details'}>
  {#if addon}
    <div class="flex flex-col gap-4">
      {#if matched?.remote?.uiIMGThumbs && matched.remote.uiIMGThumbs.length > 0}
        <div class="-mx-1 overflow-hidden rounded-lg">
          <img
            src={matched.remote.uiIMGs?.[0] ?? matched.remote.uiIMGThumbs[0]}
            alt={addon.title}
            class="h-36 w-full object-cover"
            loading="lazy"
          />
        </div>
      {/if}

      <div class="flex items-start gap-3">
        <div
          class="bg-secondary flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-lg"
        >
          {#if addon.isLibrary}
            <Library size={22} class="text-info" />
          {:else if iconUrl}
            <img
              src={iconUrl}
              alt=""
              aria-hidden="true"
              class="h-full w-full object-cover"
              loading="lazy"
            />
          {:else}
            <Package size={22} class="text-muted-foreground" />
          {/if}
        </div>

        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-2">
            <span class="text-base leading-tight font-semibold">{addon.title}</span>
            {#if addon.version || matched?.localVersion}
              <span class="text-muted-foreground font-mono text-xs"
                >v{matched?.localVersion ?? addon.version}</span
              >
            {/if}
            {#if addon.isLibrary}
              <Badge variant="outline" class="text-xs">Library</Badge>
            {/if}
          </div>
          <div class="text-muted-foreground mt-0.5 text-xs">
            {matched?.remote?.uiAuthorName || addon.author || 'Unknown Author'}
            {#if matched?.remote}
              <span class="mx-1 opacity-40">·</span>
              {#if matched.updateAvailable}
                <span class="text-destructive font-medium"
                  >Update available → {matched.remote.uiVersion}</span
                >
              {:else}
                <span>Up to date</span>
              {/if}
            {/if}
          </div>
        </div>

        {#if matched?.updateAvailable}
          <Button
            variant="default"
            size="sm"
            class="shrink-0"
            onclick={handleUpdate}
            disabled={isInstalling || remote.installing}
          >
            {#if isInstalling}
              <Loader2 size={13} class="animate-spin" />Updating...
            {:else}
              <ArrowUpCircle size={13} />Update
            {/if}
          </Button>
        {/if}

        {#if !uninstallConfirming}
          <Button
            variant="ghost"
            size="sm"
            class="text-muted-foreground hover:text-destructive shrink-0"
            onclick={requestUninstall}
            disabled={isUninstalling || remote.installing}
            aria-label="Uninstall addon"
          >
            <Trash2 size={14} />
          </Button>
        {/if}
      </div>

      {#if installError}
        <div class="border-destructive/50 bg-destructive/10 rounded-md p-3">
          <p class="text-destructive text-xs">{installError}</p>
        </div>
      {/if}

      {#if uninstallConfirming}
        <div class="border-destructive/40 bg-destructive/8 rounded-md border p-3">
          <p class="text-foreground text-sm font-medium">Uninstall this addon?</p>
          <p class="text-muted-foreground mt-0.5 text-xs">
            The folder <span class="font-mono">{addon.folderName}</span> will be permanently deleted.
          </p>
          <div class="mt-3 flex gap-2">
            <Button
              variant="destructive"
              size="sm"
              onclick={confirmUninstall}
              disabled={isUninstalling}
            >
              {#if isUninstalling}
                <Loader2 size={13} class="animate-spin" />Uninstalling...
              {:else}
                <Trash2 size={13} />Uninstall
              {/if}
            </Button>
            <Button variant="outline" size="sm" onclick={cancelUninstall} disabled={isUninstalling}>
              Cancel
            </Button>
          </div>
        </div>
      {/if}

      {#if uninstallError}
        <div class="border-destructive/50 bg-destructive/10 rounded-md p-3">
          <p class="text-destructive text-xs">{uninstallError}</p>
        </div>
      {/if}

      {#if matched?.remote}
        <StatsPill
          downloads={matched.remote.uiDownloadTotal}
          favorites={matched.remote.uiFavoriteTotal}
          linkUrl={matched.remote.uiFileInfoUrl}
        />
      {/if}

      {#if addon.description}
        <div>
          <p class="text-foreground mb-1.5 border-b pb-1 text-sm font-semibold">Description</p>
          <p class="text-foreground text-sm leading-relaxed">{addon.description}</p>
        </div>
      {/if}

      <Separator />

      <AddonMetadataGrid {addon} {matched} />

      <DependencyList
        deps={addon.dependsOn ?? []}
        {installedFolderNames}
        {depUIDMap}
        {hasCycle}
        batchInstalling={batchInstallingState}
        {batchInstallError}
        installingUID={depInstallingUID}
        installErrors={depInstallErrors}
        oninstall={installDep}
        oninstallall={missingInstallableUIDs.length > 0 ? installAllMissing : undefined}
        onsearch={searchDepInFindMore}
      />

      <OptionalDependencyList
        deps={addon.optionalDependsOn ?? []}
        {installedFolderNames}
      />

      {#if addon.savedVariables && addon.savedVariables.length > 0}
        <div>
          <p class="text-foreground mb-2 border-b pb-1 text-sm font-semibold">Saved Variables</p>
          <div class="flex flex-wrap gap-1.5">
            {#each addon.savedVariables as sv (sv)}
              <Badge variant="outline" class="font-mono text-xs">{sv}</Badge>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
</Dialog>
