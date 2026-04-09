<svelte:options runes />

<script lang="ts">
  import AlertCircle from 'lucide-svelte/icons/alert-circle';
  import AlertTriangle from 'lucide-svelte/icons/alert-triangle';
  import CheckCircle from 'lucide-svelte/icons/check-circle';
  import Download from 'lucide-svelte/icons/download';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import Package from 'lucide-svelte/icons/package';
  import Search from 'lucide-svelte/icons/search';
  import { Button, Separator } from '$lib/components/ui';

  interface Props {
    deps: string[];
    installedFolderNames: Set<string>;
    depUIDMap: Map<string, string>;
    hasCycle?: boolean;
    batchInstalling?: boolean;
    batchInstallError?: string | null;
    installingUID?: string | null;
    installErrors?: Record<string, string>;
    oninstall?: (dep: string, uid: string) => void;
    oninstallall?: () => void;
    onsearch?: (dep: string) => void;
  }

  const {
    deps,
    installedFolderNames,
    depUIDMap,
    hasCycle = false,
    batchInstalling = false,
    batchInstallError = null,
    installingUID = null,
    installErrors = {},
    oninstall,
    oninstallall,
    onsearch
  }: Props = $props();

  function depFolderName(dep: string): string {
    return dep.replace(/[><=]+\d+.*$/, '').trim();
  }

  function isInstalled(dep: string): boolean {
    return installedFolderNames.has(depFolderName(dep).toLowerCase());
  }

  function getRemoteUID(dep: string): string | null {
    return depUIDMap.get(depFolderName(dep).toLowerCase()) ?? null;
  }

  const missingUIDs = $derived(
    deps
      .filter((dep) => !isInstalled(dep))
      .map((dep) => getRemoteUID(dep))
      .filter((uid): uid is string => uid !== null)
  );
</script>

{#if deps.length > 0}
  <Separator />
  <div>
    <div class="mb-2 flex items-center justify-between border-b pb-1">
      <p class="text-foreground text-sm font-semibold">Required Dependencies</p>
      {#if missingUIDs.length > 0 && oninstallall}
        <Button
          variant="outline"
          size="sm"
          class="h-6 px-2 text-xs"
          onclick={oninstallall}
          disabled={batchInstalling}
        >
          {#if batchInstalling}
            <Loader2 size={11} class="animate-spin" />Installing...
          {:else}
            <Download size={11} />Install All Missing ({missingUIDs.length})
          {/if}
        </Button>
      {/if}
    </div>
    {#if hasCycle}
      <div
        class="border-warning/40 bg-warning/10 mb-2 flex items-center gap-2 rounded-md border p-2"
      >
        <AlertTriangle size={13} class="text-warning shrink-0" />
        <span class="text-warning text-xs"
          >Circular dependency detected in this addon's requirements.</span
        >
      </div>
    {/if}
    {#if batchInstallError}
      <div class="border-destructive/50 bg-destructive/10 mb-2 rounded-md p-2">
        <p class="text-destructive text-xs">{batchInstallError}</p>
      </div>
    {/if}
    <div class="flex flex-col gap-2">
      {#each deps as dep (dep)}
        {@const installed = isInstalled(dep)}
        {@const remoteUID = installed ? null : getRemoteUID(dep)}
        {@const isInstallingDep = installingUID === remoteUID && remoteUID !== null}
        <div class="flex items-center gap-2">
          {#if installed}
            <CheckCircle size={13} class="text-success shrink-0" />
          {:else}
            <AlertCircle size={13} class="text-destructive shrink-0" />
          {/if}
          <span class={installed ? 'text-foreground text-sm' : 'text-destructive text-sm'}
            >{dep}</span
          >
          {#if !installed}
            {#if remoteUID && oninstall}
              <Button
                variant="outline"
                size="sm"
                class="ml-auto h-6 px-2 text-xs"
                onclick={() => oninstall(dep, remoteUID)}
                disabled={isInstallingDep}
              >
                {#if isInstallingDep}
                  <Loader2 size={11} class="animate-spin" />Installing...
                {:else}
                  <Download size={11} />Install
                {/if}
              </Button>
            {:else if onsearch}
              <Button
                variant="ghost"
                size="sm"
                class="text-muted-foreground ml-auto h-6 px-2 text-xs"
                onclick={() => onsearch(dep)}
              >
                <Search size={11} />Find
              </Button>
            {/if}
            {#if installErrors[dep]}
              <span class="text-destructive text-xs">{installErrors[dep]}</span>
            {/if}
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}
