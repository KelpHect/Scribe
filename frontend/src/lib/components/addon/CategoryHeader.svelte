<svelte:options runes />

<script lang="ts">
  import ChevronRight from 'lucide-svelte/icons/chevron-right';
  import Package from 'lucide-svelte/icons/package';

  interface Props {
    name: string;
    iconUrl?: string;
    count: number;
    expanded: boolean;
    ontoggle: () => void;
  }

  const { name, iconUrl, count, expanded, ontoggle }: Props = $props();
</script>

<div class="pb-0.5">
  <button
    onclick={ontoggle}
    class="flex w-full cursor-pointer items-center gap-2.5 rounded-lg border border-[var(--color-border)] px-3 py-2 text-left transition-colors hover:bg-[var(--color-accent)]"
    aria-expanded={expanded}
  >
    <ChevronRight
      size={14}
      class="text-muted-foreground shrink-0 transition-transform duration-150 {expanded ? 'rotate-90' : ''}"
    />
    {#if iconUrl}
      <img
        src={iconUrl}
        alt=""
        aria-hidden="true"
        class="h-4 w-4 shrink-0 object-contain"
        loading="lazy"
      />
    {:else}
      <Package size={16} class="text-muted-foreground shrink-0" />
    {/if}
    <span class="flex-1 truncate text-sm font-medium">{name}</span>
    <span class="text-muted-foreground text-[11px]">{count}</span>
  </button>
</div>
