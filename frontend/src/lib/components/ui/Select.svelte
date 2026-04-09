<svelte:options runes />

<script lang="ts">
  import ChevronDown from 'lucide-svelte/icons/chevron-down';
  import { cn } from '$lib/utils';

  interface Option {
    value: string;
    label: string;
    iconUrl?: string;
  }

  interface Props {
    value: string;
    options: Option[];
    onchange: (_value: string) => void;
    placeholder?: string;
    class?: string;
    menuClass?: string;
    dark?: boolean;
    align?: 'start' | 'end';
    'aria-label'?: string;
  }

  const {
    value,
    options,
    onchange,
    placeholder = 'Select…',
    class: className,
    menuClass,
    dark = false,
    align = 'start',
    'aria-label': ariaLabel
  }: Props = $props();

  let isOpen = $state(false);
  let buttonEl = $state<HTMLButtonElement | null>(null);

  const selectedLabel = $derived(options.find((o) => o.value === value)?.label ?? placeholder);

  function toggle() {
    isOpen = !isOpen;
  }

  function select(val: string) {
    onchange(val);
    isOpen = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      isOpen = false;
      buttonEl?.focus();
    }
  }

  function handleBackdropClick() {
    isOpen = false;
  }
</script>

<div class={cn('relative', className)}>
  <button
    bind:this={buttonEl}
    onclick={toggle}
    onkeydown={handleKeydown}
    class={cn(
      'flex h-8 w-full min-w-0 cursor-pointer items-center justify-between gap-2 rounded-md border px-3 text-sm transition-colors focus:outline-none',
      dark
        ? 'border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] text-[var(--color-toolbar-input-foreground)] hover:bg-[var(--color-toolbar-accent)]'
        : 'border-border bg-background text-foreground hover:bg-accent'
    )}
    aria-haspopup="listbox"
    aria-expanded={isOpen}
    aria-label={ariaLabel}
  >
    <span class="min-w-0 truncate">{selectedLabel}</span>
    <ChevronDown
      size={13}
      class={cn(
        'shrink-0 transition-transform',
        isOpen && 'rotate-180',
        dark ? 'text-[var(--color-toolbar-muted)]' : 'text-muted-foreground'
      )}
    />
  </button>

  {#if isOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="fixed inset-0 z-40" onclick={handleBackdropClick} role="button" tabindex="-1"></div>
    <div
      class={cn(
        'bg-card border-border absolute top-full z-50 mt-1 max-h-64 min-w-[180px] overflow-y-auto rounded-lg border shadow-lg',
        align === 'end' ? 'right-0' : 'left-0',
        menuClass
      )}
      role="listbox"
      tabindex="-1"
      onkeydown={handleKeydown}
    >
      {#each options as opt (opt.value)}
        <button
          onclick={() => select(opt.value)}
          class={cn(
            'flex w-full cursor-pointer items-center px-3 py-1.5 text-left text-sm transition-colors',
            value === opt.value
              ? 'bg-primary/10 text-primary font-medium'
              : 'text-foreground hover:bg-accent'
          )}
          role="option"
          aria-selected={value === opt.value}
        >
          {#if opt.iconUrl}
            <img
              src={opt.iconUrl}
              alt=""
              aria-hidden="true"
              class="mr-2 h-4 w-4 shrink-0 object-contain"
            />
          {/if}
          <span class="truncate">{opt.label}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
