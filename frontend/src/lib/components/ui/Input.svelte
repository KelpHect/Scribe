<svelte:options runes />

<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  import { cn } from '$lib/utils';

  type Props = HTMLInputAttributes & {
    class?: string;
    ref?: HTMLInputElement;
  };

  let { class: className, ref = $bindable(), ...rest }: Props = $props();
</script>

<input
  bind:this={ref}
  class={cn(
    'border-input file:text-foreground placeholder:text-muted-foreground focus-visible:ring-ring flex h-9 w-full rounded-md border bg-transparent px-3 py-1 text-sm shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium focus-visible:ring-1 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50',
    className
  )}
  {...rest}
/>
