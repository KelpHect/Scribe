<svelte:options runes />

<script lang="ts">
  import AlertTriangle from 'lucide-svelte/icons/alert-triangle';
  import Clipboard from 'lucide-svelte/icons/clipboard';
  import RefreshCw from 'lucide-svelte/icons/refresh-cw';
  import { Button } from '$lib/components/ui';
  import type { RecoverableRouteError } from '$lib/routes/recovery';
  import { clipboardSetText } from '$lib/services/runtime-service';

  interface Props {
    title: string;
    error: RecoverableRouteError;
    onretry?: () => void | Promise<void>;
  }

  const { title, error, onretry }: Props = $props();
  let copyLabel = $state('Copy details');

  async function copyDetails() {
    const text = `${title}\n${error.message}\n\n${error.details}`;

    try {
      await clipboardSetText(text);
      copyLabel = 'Copied';
      window.setTimeout(() => {
        copyLabel = 'Copy details';
      }, 1500);
    } catch {
      copyLabel = 'Copy failed';
    }
  }
</script>

<section class="flex h-full min-h-[320px] items-center justify-center p-6" aria-live="polite">
  <div class="border-destructive/40 bg-destructive/8 max-w-xl rounded-lg border p-5 shadow-sm">
    <div class="mb-4 flex items-start gap-3">
      <AlertTriangle size={22} class="text-destructive mt-0.5 shrink-0" />
      <div class="min-w-0">
        <h2 class="text-foreground text-base font-semibold">{title}</h2>
        <p class="text-destructive mt-1 text-sm">{error.message}</p>
      </div>
    </div>

    <div class="flex flex-wrap gap-2">
      {#if onretry}
        <Button variant="outline" size="sm" onclick={onretry}>
          <RefreshCw size={13} />
          Retry
        </Button>
      {/if}
      <Button variant="ghost" size="sm" onclick={copyDetails}>
        <Clipboard size={13} />
        {copyLabel}
      </Button>
    </div>
  </div>
</section>
