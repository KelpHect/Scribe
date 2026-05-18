<svelte:options runes />

<script lang="ts">
  import { Badge } from '$lib/components/ui';
  import { cn } from '$lib/utils';
  import Trash2 from 'lucide-svelte/icons/trash-2';
  import type { Addon } from '$lib/services/addon-service';
  import FixedAddonImage from './FixedAddonImage.svelte';

  interface Props {
    addon: Addon;
    selected?: boolean;
    updateAvailable?: boolean;
    updateLabel?: string;
    updateReason?: string;
    categoryIconUrl?: string;
    isThumbnail?: boolean;
    selectable?: boolean;
    checked?: boolean;
    onclick?: () => void;
    onmenu?: (_event: MouseEvent | KeyboardEvent) => void;
    ontoggle?: () => void;
    onuninstall?: () => void;
    uninstalling?: boolean;
  }

  const {
    addon,
    selected = false,
    updateAvailable = false,
    updateLabel = 'Update',
    updateReason = '',
    categoryIconUrl,
    isThumbnail = false,
    selectable = false,
    checked = false,
    onclick,
    onmenu,
    ontoggle,
    onuninstall,
    uninstalling = false
  }: Props = $props();

  function handleKeydown(event: KeyboardEvent) {
    if ((event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) && onmenu) {
      event.preventDefault();
      onmenu(event);
      return;
    }

    if (onclick && (event.key === 'Enter' || event.key === ' ')) {
      event.preventDefault();
      onclick();
    }
  }
</script>

<div
  role="button"
  tabindex="0"
  aria-label={`View details for ${addon.title}`}
  onclick={onclick}
  oncontextmenu={onmenu}
  onkeydown={handleKeydown}
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

  <FixedAddonImage
    src={categoryIconUrl}
    thumbnail={isThumbnail}
    class={cn(categoryIconUrl ? 'bg-secondary/60' : addon.isLibrary ? 'bg-info/10' : 'bg-secondary')}
  />

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
        <span title={updateReason}>
          <Badge variant="destructive">{updateLabel}</Badge>
        </span>
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
