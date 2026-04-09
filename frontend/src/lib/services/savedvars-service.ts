import { callWails } from '$lib/services/wails-service';

export interface SearchPreset {
  id: string;
  name: string;
  searchQuery: string;
  categoryFilter: string;
  sortBy: string;
  hideInstalled: boolean;
  createdAt: string;
}


export async function listSearchPresets(): Promise<SearchPreset[]> {
  try {
    return (await callWails('ListSearchPresets')) ?? [];
  } catch {
    return [];
  }
}

export async function saveSearchPreset(
  name: string,
  searchQuery: string,
  categoryFilter: string,
  sortBy: string,
  hideInstalled: boolean
): Promise<SearchPreset> {
  return await callWails('SaveSearchPreset', name, searchQuery, categoryFilter, sortBy, hideInstalled);
}

export async function deleteSearchPreset(id: string): Promise<void> {
  await callWails('DeleteSearchPreset', id);
}
