import type { AppInfo } from '$lib/services/app-info-service';
import type { DiagnosticsSnapshot } from '$lib/services/diagnostics-service';
import type { FrontendPerformanceSnapshot } from '$lib/diagnostics/frontend-perf';
import type { TaskProgress } from '$lib/stores/downloads.svelte';

export type FrontendDiagnosticsExport = {
  addonDetailQueries: number;
  addonDetailQueriesWithData: number;
  addonDetailFresh: number;
  addonDetailStale: number;
  addonDetailScreenshotUrls: number;
  addonDetailMaxQueries: number;
  addonDetailMaxScreenshots: number;
  cachedUIDs: string[];
  performance: FrontendPerformanceSnapshot;
};

type BuildDiagnosticsExportInput = {
  appInfo: AppInfo | null;
  diagnostics: DiagnosticsSnapshot;
  frontendDiagnostics: FrontendDiagnosticsExport;
  addonPath: string;
  detectedPath: string;
  failedTasks: TaskProgress[];
  generatedAt?: string;
};

export function buildLocalDiagnosticsExport(input: BuildDiagnosticsExportInput): string {
  const payload = {
    generatedAt: input.generatedAt ?? new Date().toISOString(),
    app: {
      version: input.appInfo?.version ?? 'unknown',
      commit: input.appInfo?.commit ?? 'unknown',
      buildDate: input.appInfo?.buildDate ?? 'unknown',
      platform: input.appInfo ? `${input.appInfo.os}/${input.appInfo.arch}` : 'unknown',
      goVersion: input.appInfo?.goVersion ?? 'unknown'
    },
    paths: {
      addonPath: redactPath(input.addonPath),
      detectedPath: redactPath(input.detectedPath)
    },
    startup: {
      startupMs: input.diagnostics.startupMs,
      domReadyMs: input.diagnostics.domReadyMs,
      frontendReadyMs: input.diagnostics.frontendReadyMs,
      remoteReadyMs: input.diagnostics.remoteReadyMs,
      startupBudgetOk: input.diagnostics.startupBudgetOk ?? null
    },
    memory: {
      heapAllocMb: input.diagnostics.heapAllocMb,
      heapInUseMb: input.diagnostics.heapInUseMb ?? null,
      stackInUseMb: input.diagnostics.stackInUseMb ?? null,
      totalAllocMb: input.diagnostics.totalAllocMb ?? null,
      sysMb: input.diagnostics.sysMb,
      memoryBudgetOk: input.diagnostics.memoryBudgetOk ?? null,
      goroutines: input.diagnostics.goroutines,
      numGc: input.diagnostics.numGc
    },
    persistence: {
      status: input.diagnostics.persistenceStatus ?? 'unknown',
      error: redactLocalPathText(input.diagnostics.persistenceError ?? '')
    },
	    catalog: {
	      remoteAddons: input.diagnostics.remoteAddons,
	      remoteCategories: input.diagnostics.remoteCategories,
	      installedAddons: input.diagnostics.installedAddons,
	      remoteCacheStale: input.diagnostics.remoteCacheStale,
	      remoteRefreshCount: input.diagnostics.remoteRefreshCount,
	      lastRemoteRefreshAt: input.diagnostics.lastRemoteRefreshAt,
	      lastRemoteRefreshMs: input.diagnostics.lastRemoteRefreshMs,
	      cachedStateReadyMs: input.diagnostics.cachedStateReadyMs ?? null,
	      scanStartedMs: input.diagnostics.scanStartedMs ?? null,
	      scanReadyMs: input.diagnostics.scanReadyMs ?? null,
	      scanInFlight: input.diagnostics.scanInFlight ?? null,
	      lastScanError: redactLocalPathText(input.diagnostics.lastScanError ?? '')
	    },
    detailFetches: {
      totalBackendCalls: input.diagnostics.detailRequests,
      uniqueUIDs: input.diagnostics.detailUniqueUids,
      lastUID: input.diagnostics.lastDetailUid,
      lastAt: input.diagnostics.lastDetailAt,
      top: input.diagnostics.detailTop
    },
    frontendCache: input.frontendDiagnostics,
    recentTaskFailures: input.failedTasks.slice(0, 10).map((task) => ({
      uid: task.uid,
      name: task.name,
      state: task.state,
      percent: task.percent,
      filesExtracted: task.filesExtracted,
      totalFiles: task.totalFiles,
      error: redactLocalPathText(task.error ?? '')
    }))
  };

  return `${JSON.stringify(payload, null, 2)}\n`;
}

export function redactPath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed) return '';

  const normalized = trimmed.replace(/\\/g, '/');
  const parts = normalized.split('/').filter(Boolean);
  const leaf = parts.at(-1) ?? '';

  if (!leaf) return '[redacted-path]';
  return `[redacted-path]/${leaf}`;
}

export function redactLocalPathText(text: string): string {
  if (!text) return '';

  return text
    .replace(/[A-Za-z]:[\\/][^\s"']+/g, '[redacted-path]')
    .replace(/\\\\[^\s"']+/g, '[redacted-path]')
    .replace(/\/(?:Users|home|var|tmp|private|mnt|media|run)\/[^\s"']+/g, '[redacted-path]');
}
