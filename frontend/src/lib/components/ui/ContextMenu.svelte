<svelte:options runes />

<script lang="ts">
  import { isSeparator, type ContextMenuEntry } from '$lib/services/context-menu-service';
  import { cn } from '$lib/utils';

  interface Props {
    open: boolean;
    x: number;
    y: number;
    items: ContextMenuEntry[];
    onclose: () => void;
  }

  const { open, x, y, items, onclose }: Props = $props();

  const menuStyle = $derived(
    `left:min(${x}px, calc(100vw - 220px)); top:min(${y}px, calc(100vh - 340px));`
  );

  async function run(item: ContextMenuEntry) {
    if (isSeparator(item) || ('disabled' in item && item.disabled)) return;
    await item.action();
    onclose();
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-[90]" onclick={onclose} role="button" tabindex="-1"></div>
  <div
    class="bg-card border-border fixed z-[100] min-w-[200px] overflow-hidden rounded-lg border py-1 shadow-elevated animate-fade-in"
    style={menuStyle}
    role="menu"
  >
    {#each items as item, i (`entry-${i}`)}
      {#if isSeparator(item)}
        <div class="border-border my-1 h-px border-t"></div>
      {:else}
        <button
          type="button"
          onclick={() => run(item)}
          disabled={item.disabled}
          class={cn(
            'flex w-full cursor-pointer items-center gap-3 px-3 py-[7px] text-left text-[13px] transition-colors disabled:cursor-default disabled:opacity-40',
            !item.disabled && item.variant === 'destructive'
              ? 'text-destructive hover:bg-destructive/10'
              : 'text-foreground hover:bg-accent',
            item.disabled && 'hover:bg-transparent'
          )}
          role="menuitem"
        >
          {#if item.icon}
            <span class="flex w-4 shrink-0 items-center justify-center">
              <item.icon
                size={15}
                class={cn(
                  item.variant === 'destructive'
                    ? 'text-destructive'
                    : 'text-muted-foreground'
                )}
              />
            </span>
          {:else}
            <span class="w-4 shrink-0"></span>
          {/if}
          <span class="truncate">{item.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}
