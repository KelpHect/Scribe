<svelte:options runes />

<script lang="ts">
  import ChevronDown from 'lucide-svelte/icons/chevron-down';
  import X from 'lucide-svelte/icons/x';
  import { cn } from '$lib/utils';

  interface CategoryOption {
    id: string;
    name: string;
    iconUrl?: string;
    count: number;
    section?: string;
    indentLevel?: number;
  }

  interface Props {
    value: string[];
    options: CategoryOption[];
    totalCount: number;
    onchange: (_ids: string[]) => void;
    class?: string;
    dark?: boolean;
  }

  const { value, options, totalCount, onchange, class: className, dark = false }: Props = $props();

  let isOpen = $state(false);
  let buttonEl = $state<HTMLButtonElement | null>(null);

  const valueSet = $derived(new Set(value));
  const selectedOptions = $derived(options.filter((o) => valueSet.has(o.id)));
  const groupedOptions = $derived.by(() => {
    const groups: Array<{ section: string; total: number; options: CategoryOption[] }> = [];
    for (const option of options) {
      const section = option.section ?? 'Categories';
      const existing = groups.find((group) => group.section === section);
      if (existing) {
        existing.options.push(option);
        existing.total += option.count;
      } else {
        groups.push({ section, total: option.count, options: [option] });
      }
    }
    return groups;
  });

  function toggle() {
    isOpen = !isOpen;
  }

  function select(id: string) {
    const next = valueSet.has(id) ? value.filter((item) => item !== id) : [...value, id];
    onchange(next);
  }

  function clear(e: Event) {
    e.stopPropagation();
    onchange([]);
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
      'flex h-8 w-full min-w-0 cursor-pointer items-center gap-2 rounded-md border px-3 text-sm transition-colors focus:outline-none',
      dark
        ? 'border-[var(--color-toolbar-border)] bg-[var(--color-toolbar-input)] text-[var(--color-toolbar-input-foreground)] hover:bg-[var(--color-toolbar-accent)]'
        : 'border-border bg-background text-foreground hover:bg-accent'
    )}
    aria-haspopup="listbox"
    aria-expanded={isOpen}
  >
    {#if selectedOptions.length === 1}
      {@const selectedOption = selectedOptions[0]}
      {#if selectedOption.iconUrl}
        <img
          src={selectedOption.iconUrl}
          alt=""
          aria-hidden="true"
          class="h-4 w-4 shrink-0 object-contain"
        />
      {/if}
      <span class="min-w-0 truncate">{selectedOption.name}</span>
      <span
        class={cn(
          'shrink-0 text-xs',
          dark ? 'text-[var(--color-toolbar-muted)]' : 'text-muted-foreground'
        )}>({selectedOption.count.toLocaleString()})</span
      >
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <span
        onclick={clear}
        role="button"
        class={cn(
          '-mr-1 ml-auto shrink-0 cursor-pointer rounded p-0.5 transition-colors',
          dark
            ? 'text-[var(--color-toolbar-muted)] hover:text-[var(--color-toolbar-foreground)]'
            : 'text-muted-foreground hover:text-foreground'
        )}
        aria-label="Clear category filter"
      >
        <X size={12} />
      </span>
    {:else if selectedOptions.length > 1}
      <span class="min-w-0 truncate">{selectedOptions.length} categories selected</span>
      <span
        class={cn(
          'shrink-0 text-xs',
          dark ? 'text-[var(--color-toolbar-muted)]' : 'text-muted-foreground'
        )}>({selectedOptions.reduce((sum, option) => sum + option.count, 0).toLocaleString()})</span
      >
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <span
        onclick={clear}
        role="button"
        class={cn(
          '-mr-1 ml-auto shrink-0 cursor-pointer rounded p-0.5 transition-colors',
          dark
            ? 'text-[var(--color-toolbar-muted)] hover:text-[var(--color-toolbar-foreground)]'
            : 'text-muted-foreground hover:text-foreground'
        )}
        aria-label="Clear category filter"
      >
        <X size={12} />
      </span>
    {:else}
      <span class="min-w-0 truncate">Category: Any</span>
      <span
        class={cn(
          'shrink-0 text-xs',
          dark ? 'text-[var(--color-toolbar-muted)]' : 'text-muted-foreground'
        )}>({totalCount.toLocaleString()})</span
      >
      <ChevronDown
        size={14}
        class={cn(
          'ml-auto shrink-0',
          dark ? 'text-[var(--color-toolbar-muted)]' : 'text-muted-foreground'
        )}
      />
    {/if}
  </button>

  {#if isOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="fixed inset-0 z-40" onclick={handleBackdropClick} role="button" tabindex="-1"></div>
    <div
      class="bg-card border-border absolute top-full right-0 z-50 mt-1 max-h-72 w-64 overflow-y-auto rounded-lg border shadow-lg"
      role="listbox"
      tabindex="-1"
      onkeydown={handleKeydown}
    >
      <!-- "Any" option -->
      <button
        onclick={(e) => clear(e)}
        class={cn(
          'flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-left text-sm transition-colors',
          value.length === 0 ? 'bg-primary/10 text-primary font-medium' : 'hover:bg-accent'
        )}
        role="option"
        aria-selected={value.length === 0}
      >
        <span class="flex-1">Any Category</span>
        <span class="text-muted-foreground text-xs">{totalCount.toLocaleString()}</span>
      </button>

      <div class="bg-border mx-2 h-px"></div>

      {#each groupedOptions as group, index (group.section)}
        {#if index > 0}
          <div class="bg-border mx-2 h-px"></div>
        {/if}
        <div
          class="text-muted-foreground flex items-center justify-between px-3 pt-2 pb-1 text-[11px] font-semibold tracking-wide uppercase"
        >
          <span>{group.section}</span>
          <span>{group.total.toLocaleString()}</span>
        </div>
        {#each group.options as cat (cat.id)}
          <button
            onclick={() => select(cat.id)}
            class={cn(
              'flex w-full cursor-pointer items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors',
              valueSet.has(cat.id) ? 'bg-primary/10 text-primary font-medium' : 'hover:bg-accent'
            )}
            style={cat.indentLevel && cat.indentLevel > 0
              ? `padding-left: ${12 + cat.indentLevel * 16}px`
              : undefined}
            role="option"
            aria-selected={valueSet.has(cat.id)}
          >
            {#if cat.iconUrl}
              <img
                src={cat.iconUrl}
                alt=""
                aria-hidden="true"
                class="h-4 w-4 shrink-0 object-contain"
              />
            {:else}
              <div class="h-4 w-4 shrink-0"></div>
            {/if}
            <span class="flex-1 truncate">{cat.name}</span>
            <span class="text-muted-foreground shrink-0 text-xs">{cat.count.toLocaleString()}</span>
          </button>
        {/each}
      {/each}
    </div>
  {/if}
</div>
