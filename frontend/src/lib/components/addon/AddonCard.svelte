<svelte:options runes />

<script lang="ts">
  import { Badge } from '$lib/components/ui';
  import { cn } from '$lib/utils';
  import Package from 'lucide-svelte/icons/package';
  import Trash2 from 'lucide-svelte/icons/trash-2';
  import type { Addon } from '$lib/services/addon-service';

  interface Props {
    addon: Addon;
    selected?: boolean;
    updateAvailable?: boolean;
    categoryIconUrl?: string;
    isThumbnail?: boolean;
    selectable?: boolean;
    checked?: boolean;
    onclick?: () => void;
    ontoggle?: () => void;
    onuninstall?: () => void;
    uninstalling?: boolean;
  }

  const {
    addon,
    selected = false,
    updateAvailable = false,
    categoryIconUrl,
    isThumbnail = false,
    selectable = false,
    checked = false,
    onclick,
    ontoggle,
    onuninstall,
    uninstalling = false
  }: Props = $props();
</script>

<div
  role="button"
  tabindex="-1"
  onclick={onclick}
  class={cn(
    'flex w-full cursor-pointer items-center gap-3 rounded-lg border px-4 py-3 text-left transition-colors focus:outline-none',
    selected
      ? 'border-primary bg-accent ring-primary ring-2 ring-offset-1'
      : 'border-border bg-card hover:border-primary/50 hover:bg-accent/50'
  )}
>
  {#if selectable}
    <button
      type="button"
      class="flex h-5 w-5 shrink-0 items-center justify-center"
      aria-label={checked ? `Deselect ${addon.title}` : `Select ${addon.title}`}
      onclick={(e) => {
        e.stopPropagation();
        ontoggle?.();
      }}
    >
      <span class={cn('flex h-4 w-4 rounded border', checked ? 'border-primary bg-primary' : 'border-border bg-background')}>
        {#if checked}
          <span class="text-primary-foreground m-auto text-[10px] font-bold">✓</span>
        {/if}
      </span>
    </button>
  {/if}

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

  {#if onuninstall}
    <button
      type="button"
      class="text-muted-foreground hover:text-destructive hover:bg-destructive/10 flex h-8 w-8 shrink-0 items-center justify-center rounded-md transition-colors"
      aria-label={`Uninstall ${addon.title}`}
      disabled={uninstalling}
      onclick={(e) => {
        e.stopPropagation();
        onuninstall();
      }}
    >
      <Trash2 size={15} />
    </button>
  {/if}
</div>
