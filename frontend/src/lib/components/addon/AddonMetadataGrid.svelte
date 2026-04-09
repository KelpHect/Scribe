<svelte:options runes />

<script lang="ts">
  import FolderOpen from 'lucide-svelte/icons/folder-open';
  import Hash from 'lucide-svelte/icons/hash';
  import Tag from 'lucide-svelte/icons/tag';
  import User from 'lucide-svelte/icons/user';
  import type { Addon } from '$lib/services/addon-service';
  import type { MatchedAddon } from '$lib/services/esoui-service';

  interface Props {
    addon: Addon;
    matched?: MatchedAddon | null;
  }

  const { addon, matched = null }: Props = $props();
</script>

<div class="grid grid-cols-2 gap-x-4 gap-y-3">
  <div class="flex items-start gap-2">
    <User size={13} class="text-muted-foreground mt-0.5 shrink-0" />
    <div>
      <div class="text-muted-foreground text-xs">Author</div>
      <div class="text-foreground text-sm font-medium">
        {matched?.remote?.uiAuthorName || addon.author || 'Unknown'}
      </div>
    </div>
  </div>
  <div class="flex items-start gap-2">
    <Tag size={13} class="text-muted-foreground mt-0.5 shrink-0" />
    <div>
      <div class="text-muted-foreground text-xs">Version</div>
      <div class="text-foreground text-sm font-medium">
        {(matched?.localVersion ?? addon.version)
          ? `v${matched?.localVersion ?? addon.version}`
          : 'Unknown'}
      </div>
    </div>
  </div>
  <div class="flex items-start gap-2">
    <Hash size={13} class="text-muted-foreground mt-0.5 shrink-0" />
    <div>
      <div class="text-muted-foreground text-xs">API Version</div>
      <div class="text-foreground text-sm font-medium">{addon.apiVersion || 'N/A'}</div>
    </div>
  </div>
  <div class="flex items-start gap-2">
    <FolderOpen size={13} class="text-muted-foreground mt-0.5 shrink-0" />
    <div>
      <div class="text-muted-foreground text-xs">Folder</div>
      <div class="text-foreground text-sm font-medium break-all">{addon.folderName}</div>
    </div>
  </div>
</div>
