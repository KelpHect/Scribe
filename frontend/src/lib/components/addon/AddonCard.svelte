<svelte:options runes />

<script lang="ts">
  import { Badge } from '$lib/components/ui';
  import { cn } from '$lib/utils';
  import Package from 'lucide-svelte/icons/package';
  import type { Addon } from '$lib/services/addon-service';

  interface Props {
    addon: Addon;
    selected?: boolean;
    updateAvailable?: boolean;
    categoryIconUrl?: string;
    isThumbnail?: boolean;
    onclick?: () => void;
  }

  const {
    addon,
    selected = false,
    updateAvailable = false,
    categoryIconUrl,
    isThumbnail = false,
    onclick
  }: Props = $props();
</script>

<button
  {onclick}
  tabindex="-1"
  class={cn(
    'flex w-full cursor-pointer items-center gap-4 rounded-lg border px-4 py-3 text-left transition-colors focus:outline-none',
    selected
      ? 'border-primary bg-accent ring-primary ring-2 ring-offset-1'
      : 'border-border bg-card hover:border-primary/50 hover:bg-accent/50'
  )}
>
  <div
    class={cn(
      'flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-md',
      categoryIconUrl ? 'bg-secondary/60' : addon.isLibrary ? 'bg-info/10' : 'bg-secondary'
    )}
  >
    {#if categoryIconUrl}
      <img
        src={categoryIconUrl}
        alt=""
        aria-hidden="true"
        class={isThumbnail ? 'h-full w-full object-cover' : 'h-6 w-6 object-contain'}
        loading="lazy"
      />
    {:else}
      <Package size={20} class="text-muted-foreground" />
    {/if}
  </div>

  <div class="min-w-0 flex-1">
    <div class="flex items-center gap-2">
      <span class="truncate text-sm font-medium">{addon.title}</span>
      {#if addon.version}
        <Badge variant="secondary">v{addon.version}</Badge>
      {/if}
      {#if addon.isLibrary}
        <Badge variant="outline">Library</Badge>
      {/if}
      {#if updateAvailable}
        <Badge variant="destructive">Update</Badge>
      {/if}
    </div>
    <div class="text-muted-foreground mt-0.5 truncate text-xs">
      {#if addon.author}
        {addon.author}
      {:else}
        Unknown Author
      {/if}
      {#if addon.dependsOn && addon.dependsOn.length > 0}
        <span class="ml-2"
          >· {addon.dependsOn.length} dependenc{addon.dependsOn.length === 1 ? 'y' : 'ies'}</span
        >
      {/if}
    </div>
  </div>
</button>
