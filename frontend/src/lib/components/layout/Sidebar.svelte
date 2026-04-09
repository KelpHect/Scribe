<svelte:options runes />

<script lang="ts">
  import Download from 'lucide-svelte/icons/download';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import Package from 'lucide-svelte/icons/package';
  import Search from 'lucide-svelte/icons/search';
  import Settings from 'lucide-svelte/icons/settings';
  import { navigation, type Page, getRemoteStore, getDownloadStore } from '$lib/stores';
  import { cn } from '$lib/utils';

  type NavItem = {
    id: Page;
    label: string;
    icon: typeof Package;
    shortcut?: string;
  };

  const items: NavItem[] = [
    { id: 'installed', label: 'Installed', icon: Package, shortcut: 'Ctrl+1' },
    { id: 'find-more', label: 'Find More', icon: Search, shortcut: 'Ctrl+2' },
    { id: 'updates', label: 'Updates', icon: Download }
  ];

  const remote = getRemoteStore();
  const downloads = getDownloadStore();
</script>

<nav class="bg-sidebar border-sidebar-border flex h-full w-52 shrink-0 flex-col border-r">
  <div class="flex flex-col gap-1 p-2 pt-3">
    <div
      class="text-sidebar-muted px-3 pt-2 pb-1 text-[10px] font-semibold tracking-widest uppercase"
    >
      Addons
    </div>
    {#each items as item (item.id)}
      {@const Icon = item.icon}
      {@const active = navigation.isCurrent(item.id)}
      <button
        onclick={() => navigation.navigate(item.id)}
        onmouseenter={() => navigation.preload(item.id)}
        onfocus={() => navigation.preload(item.id)}
        class={cn(
          'group relative flex w-full cursor-pointer items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors',
          active
            ? 'bg-sidebar-accent text-sidebar-accent-foreground'
            : 'text-sidebar-foreground hover:bg-sidebar-accent/50 hover:text-sidebar-accent-foreground'
        )}
        aria-current={active ? 'page' : undefined}
      >
        {#if active}
          <span
            class="absolute top-1/2 left-0 h-5 w-[3px] -translate-y-1/2 rounded-r-full bg-[var(--color-sidebar-primary)]"
          ></span>
        {/if}

        <Icon
          size={17}
          class={active ? 'text-[var(--color-sidebar-primary)]' : 'text-sidebar-foreground'}
        />
        <span>{item.label}</span>

        {#if item.id === 'updates' && remote.updateCount > 0}
          <span
            class="bg-destructive text-destructive-foreground ml-auto flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-[10px] font-semibold tabular-nums"
          >
            {remote.updateCount}
          </span>
        {:else if item.id === 'find-more' && downloads.isDownloading}
          <span class="ml-auto flex items-center gap-1">
            <Loader2 size={12} class="text-primary animate-spin" />
            <span class="text-primary text-[10px] font-semibold tabular-nums"
              >{downloads.activeCount}</span
            >
          </span>
        {:else if item.shortcut}
          <span
            class="text-sidebar-muted ml-auto text-[10px] opacity-60 transition-opacity group-hover:opacity-100"
          >
            {item.shortcut}
          </span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="mt-auto p-2">
    <div class="mb-1 h-px bg-[var(--color-sidebar-border)]"></div>
    <button
      onclick={() => navigation.navigate('settings')}
      onmouseenter={() => navigation.preload('settings')}
      onfocus={() => navigation.preload('settings')}
      class={cn(
        'group relative flex w-full cursor-pointer items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors',
        navigation.isCurrent('settings')
          ? 'bg-sidebar-accent text-sidebar-accent-foreground'
          : 'text-sidebar-foreground hover:bg-sidebar-accent/50 hover:text-sidebar-accent-foreground'
      )}
    >
      {#if navigation.isCurrent('settings')}
        <span
          class="absolute top-1/2 left-0 h-5 w-[3px] -translate-y-1/2 rounded-r-full bg-[var(--color-sidebar-primary)]"
        ></span>
      {/if}
      <Settings
        size={17}
        class={navigation.isCurrent('settings')
          ? 'text-[var(--color-sidebar-primary)]'
          : 'text-sidebar-foreground'}
      />
      <span>Settings</span>
    </button>
  </div>
</nav>
