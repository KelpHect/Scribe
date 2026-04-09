<svelte:options runes />

<script lang="ts">
  import AlertTriangle from 'lucide-svelte/icons/alert-triangle';
  import RefreshCw from 'lucide-svelte/icons/refresh-cw';
  import { Button } from '$lib/components/ui';

  interface Props {
    error: string | null;
    onretry?: () => void;
    children: any;
  }

  const { error, onretry, children }: Props = $props();
</script>

{#if error}
  <div
    class="border-destructive/50 bg-destructive/10 flex items-center justify-between gap-3 rounded-lg border p-3"
  >
    <div class="flex items-center gap-2">
      <AlertTriangle size={16} class="text-destructive shrink-0" />
      <span class="text-destructive text-sm">{error}</span>
    </div>
    {#if onretry}
      <Button variant="outline" size="sm" onclick={onretry}>
        <RefreshCw size={13} />
        Retry
      </Button>
    {/if}
  </div>
{:else}
  {@render children()}
{/if}
