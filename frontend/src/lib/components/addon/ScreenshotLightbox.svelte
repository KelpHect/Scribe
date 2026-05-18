<svelte:options runes />

<script lang="ts">
  import ChevronLeft from 'lucide-svelte/icons/chevron-left';
  import ChevronRight from 'lucide-svelte/icons/chevron-right';
  import X from 'lucide-svelte/icons/x';

  interface Props {
    screenshots: Array<{ thumb: string; full: string }>;
    index: number;
    onclose: () => void;
    onprev: () => void;
    onnext: () => void;
  }

  const { screenshots, index, onclose, onprev, onnext }: Props = $props();
  let dialogEl = $state<HTMLDivElement | null>(null);

  const src = $derived(screenshots[index]?.full ?? null);

  $effect(() => {
    if (src === null) return;
    queueMicrotask(() => dialogEl?.focus());
  });

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
    else if (e.key === 'ArrowLeft') onprev();
    else if (e.key === 'ArrowRight') onnext();
  }
</script>

{#if src !== null}
  <!-- Dialog container handles Escape/arrow keys and backdrop clicks; buttons provide explicit keyboard controls. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={dialogEl}
    role="dialog"
    aria-modal="true"
    aria-label="Screenshot lightbox"
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/85 p-4 backdrop-blur-sm"
    onclick={onclose}
    onkeydown={handleKey}
  >
    <button
      type="button"
      class="absolute top-4 right-4 z-10 rounded-full bg-white/10 p-2 text-white transition-colors hover:bg-white/25"
      onclick={onclose}
      aria-label="Close lightbox"
    >
      <X size={18} />
    </button>

    {#if screenshots.length > 1}
      <button
        type="button"
        class="absolute top-1/2 left-4 z-10 -translate-y-1/2 rounded-full bg-white/10 p-2.5 text-white transition-colors hover:bg-white/25"
        onclick={(e) => { e.stopPropagation(); onprev(); }}
        aria-label="Previous screenshot"
      >
        <ChevronLeft size={22} />
      </button>
      <button
        type="button"
        class="absolute top-1/2 right-4 z-10 -translate-y-1/2 rounded-full bg-white/10 p-2.5 text-white transition-colors hover:bg-white/25"
        onclick={(e) => { e.stopPropagation(); onnext(); }}
        aria-label="Next screenshot"
      >
        <ChevronRight size={22} />
      </button>
    {/if}

    <!-- Image consumes pointer clicks so the backdrop does not close; it is not an interactive control. -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <img
      src={src}
      alt="Screenshot {index + 1}"
      class="max-h-[88vh] max-w-[calc(100vw-8rem)] rounded-lg object-contain shadow-2xl"
      onclick={(e) => e.stopPropagation()}
    />

    {#if screenshots.length > 1}
      <div
        class="absolute bottom-4 left-1/2 -translate-x-1/2 rounded-full bg-black/50 px-3 py-1 text-xs text-white"
      >
        {index + 1} / {screenshots.length}
      </div>
    {/if}
  </div>
{/if}
