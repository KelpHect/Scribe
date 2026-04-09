import { callWails } from '$lib/services/wails-service';

export interface Addon {
  id: string;
  folderName: string;
  title: string;
  version: string;
  author: string;
  description: string;
  dependsOn: string[];
  optionalDependsOn: string[];
  savedVariables: string[];
  apiVersion: string;
  addOnVersion: string;
  isLibrary: boolean;
  enabled: boolean;
  path: string;
}

export async function fetchInstalledAddons(): Promise<Addon[]> {
  try {
    return await callWails('GetInstalledAddons');
  } catch {
    return [];
  }
}

export async function fetchAddonPath(): Promise<string> {
  try {
    return await callWails('GetAddonPath');
  } catch {
    return '';
  }
}

export async function updateAddonPath(path: string): Promise<void> {
  await callWails('SetAddonPath', path);
}

export async function fetchDetectedPath(): Promise<string> {
  try {
    return await callWails('DetectAddonPath');
  } catch {
    return '';
  }
}

export async function browseForFolder(title: string): Promise<string> {
  try {
    return await callWails('BrowseFolder', title);
  } catch {
    return '';
  }
}

export async function openPath(path: string): Promise<void> {
  await callWails('OpenPath', path);
}
