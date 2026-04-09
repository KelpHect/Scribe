<svelte:options runes={true} />

<script lang="ts">
  import { quitApp, windowMinimise, windowToggleMaximise } from '$lib/services/runtime-service';
  import Minus from 'lucide-svelte/icons/minus';
  import Square from 'lucide-svelte/icons/square';
  import X from 'lucide-svelte/icons/x';

  type Props = {
    title?: string;
  };

  const { title = 'Scribe' }: Props = $props();
</script>

<div
  class="border-sidebar-border bg-titlebar flex h-9 shrink-0 items-center justify-between border-b select-none"
  style="--wails-draggable:drag"
>
  <div class="flex items-center gap-2 pl-3">
    <span class="text-titlebar-foreground text-xs font-medium tracking-wide">{title}</span>
  </div>

  <div class="flex h-full" style="--wails-draggable:no-drag">
    <button
      onclick={() => void windowMinimise()}
      class="text-titlebar-foreground hover:bg-sidebar-accent inline-flex h-full w-11 items-center justify-center transition-colors"
      aria-label="Minimize"
    >
      <Minus size={14} />
    </button>
    <button
      onclick={() => void windowToggleMaximise()}
      class="text-titlebar-foreground hover:bg-sidebar-accent inline-flex h-full w-11 items-center justify-center transition-colors"
      aria-label="Maximize"
    >
      <Square size={12} />
    </button>
    <button
      onclick={() => void quitApp()}
      class="text-titlebar-foreground hover:bg-destructive hover:text-destructive-foreground inline-flex h-full w-11 items-center justify-center transition-colors"
      aria-label="Close"
    >
      <X size={14} />
    </button>
  </div>
</div>
