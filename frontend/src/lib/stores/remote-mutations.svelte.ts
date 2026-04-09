import { refreshInstalledState } from '$lib/db/query-state';
import { uninstallAddon } from '$lib/services/esoui-service';
import { toast } from 'svelte-sonner';

export async function uninstallRemoteAddons(
  addons: Array<{ folderName: string; displayName?: string }>
): Promise<void> {
  if (addons.length === 0) return;

  for (const addon of addons) {
    await uninstallAddon(addon.folderName);
  }

  await refreshInstalledState();

  if (addons.length === 1) {
    const label = addons[0].displayName ?? addons[0].folderName;
    toast.success(`${label} uninstalled`, { duration: 4000 });
    return;
  }

  toast.success(`${addons.length} addons uninstalled`, { duration: 4000 });
}

export async function uninstallRemoteAddon(folderName: string, displayName?: string): Promise<void> {
  await uninstallRemoteAddons([{ folderName, displayName }]);
}
