import { callWails } from '$lib/services/wails-service';

export interface DiagnosticsCount {
  name: string;
  count: number;
}

export interface DiagnosticsSnapshot {
  startupMs: number;
  domReadyMs: number;
  frontendReadyMs: number;
  remoteReadyMs: number;
  heapAllocMb: number;
  sysMb: number;
  goroutines: number;
  numGc: number;
  remoteAddons: number;
  remoteCategories: number;
  installedAddons: number;
  remoteCacheStale: boolean;
  detailRequests: number;
  detailUniqueUids: number;
  lastDetailUid: string;
  lastDetailAt: string;
  detailTop: DiagnosticsCount[];
  remoteRefreshCount: number;
  lastRemoteRefreshAt: string;
  lastRemoteRefreshMs: number;
  heapInUseMb?: number;
  stackInUseMb?: number;
  totalAllocMb?: number;
  memoryBudgetOk?: boolean;
  startupBudgetOk?: boolean;
}

export async function fetchDiagnostics(): Promise<DiagnosticsSnapshot> {
  return await callWails('GetDiagnostics');
}

export async function performMemoryCleanup(): Promise<DiagnosticsSnapshot> {
  return await callWails('PerformMemoryCleanup');
}
