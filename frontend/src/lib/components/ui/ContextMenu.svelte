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
  let menuEl = $state<HTMLDivElement | null>(null);

  const menuStyle = $derived(
    `left:min(${x}px, calc(100vw - 220px)); top:min(${y}px, calc(100vh - 340px));`
  );

  $effect(() => {
    if (!open) return;
    queueMicrotask(() => firstEnabledItem()?.focus());
  });

  function enabledItems() {
    return Array.from(menuEl?.querySelectorAll<HTMLButtonElement>('button[data-menuitem]') ?? []).filter(
      (button) => !button.disabled
    );
  }

  function firstEnabledItem() {
    return enabledItems()[0] ?? null;
  }

  async function run(item: ContextMenuEntry) {
    if (isSeparator(item) || ('disabled' in item && item.disabled)) return;
    await item.action();
    onclose();
  }

  function focusItem(offset: number) {
    const buttons = enabledItems();
    if (buttons.length === 0) return;
    const currentIndex = buttons.findIndex((button) => button === document.activeElement);
    const nextIndex = currentIndex === -1 ? 0 : (currentIndex + offset + buttons.length) % buttons.length;
    buttons[nextIndex]?.focus();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onclose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusItem(1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusItem(-1);
    } else if (e.key === 'Home') {
      e.preventDefault();
      firstEnabledItem()?.focus();
    } else if (e.key === 'End') {
      e.preventDefault();
      enabledItems().at(-1)?.focus();
    }
  }
</script>

{#if open}
  <!-- Backdrop click closes the menu; keyboard users use Escape inside the focused menu. -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="fixed inset-0 z-[90]" onclick={onclose} role="button" tabindex="-1"></div>
  <div
    bind:this={menuEl}
    class="bg-card border-border fixed z-[100] min-w-[200px] overflow-hidden rounded-lg border py-1 shadow-elevated animate-fade-in"
    style={menuStyle}
    role="menu"
    tabindex="-1"
    onkeydown={handleKeydown}
  >
    {#each items as item, i (`entry-${i}`)}
      {#if isSeparator(item)}
        <div class="border-border my-1 h-px border-t"></div>
      {:else}
        <button
          type="button"
          data-menuitem
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
