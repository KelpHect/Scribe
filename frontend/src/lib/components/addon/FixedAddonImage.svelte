<svelte:options runes />

<script lang="ts">
  import Package from 'lucide-svelte/icons/package';
  import { cn } from '$lib/utils';

  interface Props {
    src?: string | null;
    alt?: string;
    thumbnail?: boolean;
    size?: 'sm' | 'md';
    class?: string;
    imageClass?: string;
    fallbackClass?: string;
  }

  const {
    src,
    alt = '',
    thumbnail = false,
    size = 'md',
    class: className = '',
    imageClass = '',
    fallbackClass = 'text-muted-foreground'
  }: Props = $props();

  let failedSrc = $state<string | null>(null);

  const boxSizeClass = $derived(size === 'sm' ? 'h-4 w-4' : 'h-10 w-10');
  const imageSize = $derived(size === 'sm' ? 16 : 40);
  const fallbackSize = $derived(size === 'sm' ? 16 : 20);
  const hasImage = $derived(!!src && failedSrc !== src);
</script>

<div
  class={cn(
    'bg-secondary flex shrink-0 items-center justify-center overflow-hidden rounded-md',
    boxSizeClass,
    className
  )}
>
  {#if hasImage}
    <img
      src={src}
      alt={alt}
      aria-hidden={alt ? undefined : 'true'}
      width={imageSize}
      height={imageSize}
      class={cn(thumbnail ? 'h-full w-full object-cover' : 'h-3/5 w-3/5 object-contain', imageClass)}
      loading="lazy"
      decoding="async"
	      draggable="false"
	      referrerpolicy="no-referrer"
	      onerror={() => (failedSrc = src ?? null)}
	    />
  {:else}
    <Package size={fallbackSize} class={fallbackClass} />
  {/if}
</div>
