<svelte:options runes />

<script lang="ts">
  import { cn } from '$lib/utils';

  interface Props {
    class?: string;
  }

  const { class: className }: Props = $props();
</script>

<div class={cn('bg-muted animate-pulse rounded-md', className)}></div>
