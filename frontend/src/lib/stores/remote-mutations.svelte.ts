import { refreshInstalledState } from '$lib/db/query-state';
import { uninstallAddon } from '$lib/services/esoui-service';
import { toast } from 'svelte-sonner';

export async function uninstallRemoteAddon(folderName: string, displayName?: string): Promise<void> {
  await uninstallAddon(folderName);
  await refreshInstalledState();

  const label = displayName ?? folderName;
  toast.success(`${label} uninstalled`, { duration: 4000 });
}
