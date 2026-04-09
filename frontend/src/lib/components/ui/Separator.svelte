<svelte:options runes />

<script lang="ts">
  import { cn } from '$lib/utils';

  type Orientation = 'horizontal' | 'vertical';

  type Props = {
    orientation?: Orientation;
    class?: string;
  };

  const { orientation = 'horizontal', class: className }: Props = $props();
</script>

<div
  role="separator"
  class={cn(
    'bg-border shrink-0',
    orientation === 'horizontal' ? 'h-[1px] w-full' : 'h-full w-[1px]',
    className
  )}
></div>
