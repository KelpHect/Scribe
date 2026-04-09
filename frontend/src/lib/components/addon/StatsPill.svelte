<svelte:options runes />

<script lang="ts">
  import Download from 'lucide-svelte/icons/download';
  import ExternalLink from 'lucide-svelte/icons/external-link';
  import Star from 'lucide-svelte/icons/star';
  import { formatCompact } from '$lib/utils';
  import { openExternalURL } from '$lib/services/runtime-service';

  interface Props {
    downloads?: number;
    favorites?: number;
    linkUrl?: string;
    linkLabel?: string;
    extra?: any;
  }

  const {
    downloads: dlCount = 0,
    favorites: favCount = 0,
    linkUrl,
    linkLabel = 'ESOUI',
    extra
  }: Props = $props();

  async function openLink(e: MouseEvent) {
    e.preventDefault();
    if (!linkUrl) return;
    await openExternalURL(linkUrl);
  }
</script>

<div
  class="bg-muted/40 border-border flex flex-wrap items-center gap-x-5 gap-y-1.5 rounded-lg border px-4 py-2 text-xs"
>
  {#if dlCount > 0}
    <div class="flex items-center gap-1.5">
      <Download size={11} class="text-primary" />
      <span class="text-foreground font-semibold">{formatCompact(dlCount)}</span>
      <span class="text-muted-foreground">downloads</span>
    </div>
  {/if}
  {#if favCount > 0}
    <div class="flex items-center gap-1.5">
      <Star size={11} class="text-primary" />
      <span class="text-foreground font-semibold">{formatCompact(favCount)}</span>
      <span class="text-muted-foreground">favorites</span>
    </div>
  {/if}
  {#if extra}
    {@render extra()}
  {/if}
  {#if linkUrl}
    <a
      href={linkUrl}
      target="_blank"
      rel="noopener noreferrer"
      onclick={openLink}
      class="text-muted-foreground hover:text-foreground ml-auto flex items-center gap-1 transition-colors"
    >
      <ExternalLink size={10} />{linkLabel}
    </a>
  {/if}
</div>
