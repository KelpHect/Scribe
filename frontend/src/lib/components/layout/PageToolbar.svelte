<svelte:options runes />

<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    subtitle?: string;
    icon?: Snippet;
    actions?: Snippet;
    extra?: Snippet;
    filters?: Snippet;
  }

  const { title, subtitle, icon, actions, extra, filters }: Props = $props();
</script>

<div class="toolbar-band flex flex-col gap-2 px-4 pt-3 pb-3">
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-2">
      {#if icon}
        <div
          class="flex h-7 w-7 items-center justify-center rounded-md bg-[var(--color-toolbar-accent)]"
        >
          {@render icon()}
        </div>
      {/if}
      <div>
        <h1 class="text-sm leading-tight font-semibold text-[var(--color-toolbar-foreground)]">
          {title}
        </h1>
        {#if subtitle}
          <p class="text-[11px] leading-tight text-[var(--color-toolbar-muted)]">
            {subtitle}
          </p>
        {/if}
      </div>
    </div>
    {#if actions}
      <div class="flex items-center gap-1.5">
        {@render actions()}
      </div>
    {/if}
  </div>

  {#if extra}
    {@render extra()}
  {/if}

  {#if filters}
    <div class="flex min-w-0 items-center gap-2">
      {@render filters()}
    </div>
  {/if}
</div>

<style>
  .toolbar-band {
    background-color: var(--color-toolbar);
    border-bottom: 1px solid var(--color-toolbar-border);
  }
</style>
