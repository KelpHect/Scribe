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
}

function normalizeMatchedAddon(match: WailsEsoui.MatchedAddon): MatchedAddon {
  return {
    ...match,
    remote: match.remote ?? null,
    details: match.details ?? null
  };
}


export async function fetchRemoteAddons(): Promise<RemoteAddon[]> {
  return (await callWails('GetRemoteAddons')) ?? [];
}

export async function refreshRemoteAddons(): Promise<RemoteAddon[]> {
  try {
    return (await callWails('RefreshRemoteAddons')) ?? [];
  } catch {
    return [];
  }
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
  remoteUID: string;
  remoteName: string;
  canInstall: boolean;
  optional: boolean;
}

export async function fetchMissingDependencies(): Promise<MissingDepInfo[]> {
  try {
    return (await callWails('GetMissingDependencies')) ?? [];
  } catch {
    return [];
  }
}
