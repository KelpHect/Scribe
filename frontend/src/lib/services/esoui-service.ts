import { callWails } from '$lib/services/wails-service';
import type { esoui as WailsEsoui } from '../../../wailsjs/go/models';

export interface GameVersion {
  version: string;
  name: string;
}

export interface RemoteAddon {
  uid: string;
  categoryId: string;
  uiName: string;
  uiAuthorName: string;
  uiDate: string;
  uiVersion: string;
  uiDirs: string[];
  uiFileInfoUrl: string;
  uiDownloadTotal: number;
  uiFavoriteTotal: number;
  uiIMGThumbs: string[];
  uiIMGs: string[];
  compatabilities: GameVersion[];
  siblings: string[];
}

export interface RemoteAddonDetails extends RemoteAddon {
  uiMD5: string;
  uiFileName: string;
  uiDownload: string;
  uiDescription: string;
  uiChangeLog: string;
  uiHitCount: number;
  uiDonationLink: string;
  UIPending: boolean;
  uiCatId: string;
}

export interface MatchedAddon {
  folderName: string;
  remote: RemoteAddon | null;
  details: RemoteAddonDetails | null;
  updateAvailable: boolean;
  localVersion: string;
  remoteVersion: string;
  updateState: string;
  updateReason: string;
}

export interface RemoteCatalogStatus {
  hasData: boolean;
  cacheStale: boolean;
  lastRefreshError: string;
  refreshInFlight: boolean;
  refreshStartedAt: string;
}

function normalizeMatchedAddon(match: WailsEsoui.MatchedAddon): MatchedAddon {
  const withState = match as WailsEsoui.MatchedAddon & {
    updateState?: string;
    updateReason?: string;
  };
  return {
    ...match,
    remote: match.remote ?? null,
    details: match.details ?? null,
    updateState: withState.updateState ?? (match.updateAvailable ? 'remote-newer' : 'up-to-date'),
    updateReason: withState.updateReason ?? ''
  };
}


export async function fetchRemoteAddons(): Promise<RemoteAddon[]> {
  return (await callWails('GetRemoteAddons')) ?? [];
}

export async function fetchRemoteCatalogStatus(): Promise<RemoteCatalogStatus> {
  const status = (await callWails('GetRemoteCatalogStatus')) as RemoteCatalogStatus | undefined;
  return {
    hasData: status?.hasData ?? false,
    cacheStale: status?.cacheStale ?? false,
    lastRefreshError: status?.lastRefreshError ?? '',
    refreshInFlight: status?.refreshInFlight ?? false,
    refreshStartedAt: status?.refreshStartedAt ?? ''
  };
}

export async function refreshRemoteAddons(): Promise<RemoteAddon[]> {
  return (await callWails('RefreshRemoteAddons')) ?? [];
}

export async function searchRemoteAddons(query: string): Promise<RemoteAddon[]> {
  try {
    return (await callWails('SearchRemoteAddons', query)) ?? [];
  } catch {
    return [];
  }
}

export async function fetchAddonDetails(uid: string): Promise<RemoteAddonDetails | null> {
  try {
    return await callWails('GetAddonDetails', uid);
  } catch {
    return null;
  }
}

export async function checkForUpdates(): Promise<MatchedAddon[]> {
  try {
    return ((await callWails('CheckForUpdates')) ?? []).map(normalizeMatchedAddon);
  } catch {
    return [];
  }
}

export async function fetchMatchedAddons(): Promise<MatchedAddon[]> {
  try {
    return ((await callWails('GetMatchedAddons')) ?? []).map(normalizeMatchedAddon);
  } catch {
    return [];
  }
}

export async function downloadAndInstall(uid: string): Promise<void> {
  await callWails('DownloadAndInstall', uid);
}

export async function uninstallAddon(folderName: string): Promise<void> {
  await callWails('UninstallAddon', folderName);
}

export interface Category {
  id: string;
  name: string;
  iconUrl: string;
  parentId: string;
  parentIds: string[];
  count: number;
}

export async function fetchCategories(): Promise<Category[]> {
  return (await callWails('GetCategories')) ?? [];
}

export interface MissingDepInfo {
  depFolderName: string;
  requiredBy: string[];
  versionConstraints: string[];
  remoteUID: string;
  remoteName: string;
  canInstall: boolean;
  optional: boolean;
  planState: string;
  planReason: string;
}

export async function fetchMissingDependencies(): Promise<MissingDepInfo[]> {
  try {
    return ((await callWails('GetMissingDependencies')) ?? []).map((dep) => ({
      ...dep,
      requiredBy: dep.requiredBy ?? [],
      versionConstraints: (dep as MissingDepInfo).versionConstraints ?? [],
      planState: (dep as MissingDepInfo).planState ?? (dep.canInstall ? 'installable' : 'unresolved'),
      planReason:
        (dep as MissingDepInfo).planReason ??
        (dep.canInstall
          ? 'Matched the latest canonical ESOUI addon entry; dependency version constraints are informational and do not pin downloads.'
          : 'No ESOUI catalog entry matched this dependency folder.')
    }));
  } catch {
    return [];
  }
}
