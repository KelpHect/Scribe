<svelte:options runes />

<script lang="ts">
  import { Separator } from '$lib/components/ui';
  import X from 'lucide-svelte/icons/x';
  import type { Snippet } from 'svelte';
  import type { HTMLAttributes } from 'svelte/elements';
  import { cn } from '$lib/utils';

  interface Props extends HTMLAttributes<HTMLDivElement> {
    open: boolean;
    onclose: () => void;
    title: string;
    panelClass?: string;
    children: Snippet;
  }

  const { open, onclose, title, panelClass, children, ...rest }: Props = $props();

  $effect(() => {
    if (open) {
      document.body.style.overflow = 'hidden';
      return () => {
        document.body.style.overflow = '';
      };
    }
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center" onkeydown={onKeydown} {...rest}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="fixed inset-0 bg-black/60" onclick={onclose} role="button" tabindex="-1"></div>
    <div
      class={cn(
        'bg-card border-border animate-fade-in relative z-10 flex max-h-[85vh] w-full flex-col rounded-xl border shadow-2xl',
        panelClass ?? 'max-w-lg'
      )}
    >
      <div class="flex items-center justify-between px-6 py-4">
        <h2 class="text-lg font-semibold">{title}</h2>
        <button
          onclick={onclose}
          class="text-muted-foreground hover:text-foreground cursor-pointer rounded-md p-1 transition-colors"
        >
          <X size={18} />
        </button>
      </div>
      <Separator />
      <div class="overflow-y-auto px-6 py-4">
        {@render children()}
      </div>
    </div>
  </div>
{/if}
