<svelte:options runes />

<script lang="ts">
  import CheckCircle from 'lucide-svelte/icons/check-circle';
  import Package from 'lucide-svelte/icons/package';

  interface Props {
    deps: string[];
    installedFolderNames: Set<string>;
  }

  const { deps, installedFolderNames }: Props = $props();

  function depFolderName(dep: string): string {
    return dep.replace(/[><=]+\d+.*$/, '').trim();
  }
</script>

{#if deps.length > 0}
  <div>
    <p class="text-foreground mb-2 border-b pb-1 text-sm font-semibold">
      Optional Dependencies
    </p>
    <div class="flex flex-col gap-1.5">
      {#each deps as dep (dep)}
        {@const installed = installedFolderNames.has(depFolderName(dep).toLowerCase())}
        <div class="flex items-center gap-2">
          {#if installed}
            <CheckCircle size={13} class="text-success shrink-0" />
          {:else}
            <Package size={13} class="text-muted-foreground shrink-0" />
          {/if}
          <span class="text-foreground text-sm">{dep}</span>
          {#if !installed}
            <span class="text-muted-foreground text-xs">— not installed</span>
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}
