import { describe, expect, it } from 'vitest';
import { buildLocalDiagnosticsExport, redactLocalPathText, redactPath } from './export';

describe('diagnostics export redaction', () => {
  it('keeps only the final path segment for addon paths', () => {
    expect(redactPath('/home/alice/Documents/Elder Scrolls Online/live/AddOns')).toBe(
      '[redacted-path]/AddOns'
    );
    expect(redactPath('C:\\Users\\Alice\\Documents\\Elder Scrolls Online\\live\\AddOns')).toBe(
      '[redacted-path]/AddOns'
    );
  });

  it('redacts local paths from task errors', () => {
    expect(redactLocalPathText('failed to open /home/alice/AddOns/LibFoo/Foo.txt')).toBe(
      'failed to open [redacted-path]'
    );
    expect(redactLocalPathText('failed C:\\Users\\Alice\\AddOns\\LibFoo\\Foo.txt')).toBe(
      'failed [redacted-path]'
    );
  });

  it('exports app, platform, cache, memory, persistence, and recent task failures', () => {
    const exported = buildLocalDiagnosticsExport({
      generatedAt: '2026-05-18T12:00:00.000Z',
      appInfo: {
        version: '1.0.3',
        commit: 'abc123',
        buildDate: '2026-05-18',
        goVersion: 'go1.26.3',
        os: 'linux',
        arch: 'amd64'
      },
      addonPath: '/home/alice/ESO/live/AddOns',
      detectedPath: '/home/alice/ESO/live/AddOns',
	      frontendDiagnostics: {
	        addonDetailQueries: 2,
	        addonDetailQueriesWithData: 1,
	        addonDetailFresh: 1,
	        addonDetailStale: 0,
	        cachedUIDs: ['101'],
	        performance: {
	          timings: [
	            {
	              name: 'findMore.filterSort',
	              count: 1,
	              lastMs: 3.2,
	              avgMs: 3.2,
	              maxMs: 3.2,
	              p95Ms: 3.2,
	              lastAt: '2026-05-18T12:00:00Z',
	              meta: { resultCount: 10 }
	            }
	          ],
	          gauges: [
	            {
	              name: 'findMore.visibleItems',
	              value: 12,
	              updatedAt: '2026-05-18T12:00:00Z',
	              meta: { resultCount: 10 }
	            }
	          ],
	          progressEvents: {
	            totalEvents: 4,
	            lastMinuteEvents: 4,
	            lastEventAt: '2026-05-18T12:00:00Z',
	            lastState: 'downloading',
	            lastUID: '101',
	            lastTaskCount: 1,
	            lastActiveCount: 1,
	            errorEvents: 0,
	            droppedEvents: 0
	          }
	        }
	      },
      diagnostics: {
        startupMs: 100,
        domReadyMs: 80,
        frontendReadyMs: 120,
        remoteReadyMs: 300,
        heapAllocMb: 10,
        sysMb: 80,
        goroutines: 8,
        numGc: 1,
        remoteAddons: 5000,
        remoteCategories: 20,
        installedAddons: 40,
        remoteCacheStale: false,
        detailRequests: 3,
        detailUniqueUids: 2,
        lastDetailUid: '101',
        lastDetailAt: '2026-05-18T12:00:00Z',
        detailTop: [{ name: '101', count: 2 }],
        remoteRefreshCount: 1,
        lastRemoteRefreshAt: '2026-05-18T12:00:00Z',
        lastRemoteRefreshMs: 250,
        heapInUseMb: 12,
        stackInUseMb: 1,
        totalAllocMb: 30,
        memoryBudgetOk: true,
        startupBudgetOk: true,
        persistenceStatus: 'ok',
        persistenceError: '',
        cachedStateReadyMs: 20,
        scanStartedMs: 30,
        scanReadyMs: 60,
        scanInFlight: false,
        lastScanError: ''
      },
      failedTasks: [
        {
          uid: '101',
          name: 'LibFoo',
          state: 'failed',
          percent: 50,
          bytesDownloaded: 10,
          totalBytes: 20,
          speed: 1,
          error: 'failed to write /home/alice/ESO/live/AddOns/LibFoo/Foo.lua',
          filesExtracted: 1,
          totalFiles: 2,
          queuePosition: 0
        }
      ]
    });

    const payload = JSON.parse(exported);
    expect(payload.app).toMatchObject({ version: '1.0.3', platform: 'linux/amd64' });
    expect(payload.paths.addonPath).toBe('[redacted-path]/AddOns');
    expect(payload.memory.sysMb).toBe(80);
	    expect(payload.persistence.status).toBe('ok');
	    expect(payload.catalog.remoteAddons).toBe(5000);
	    expect(payload.catalog.scanReadyMs).toBe(60);
	    expect(payload.frontendCache.performance.timings[0].name).toBe('findMore.filterSort');
	    expect(payload.frontendCache.performance.progressEvents.totalEvents).toBe(4);
	    expect(payload.recentTaskFailures[0].error).toBe('failed to write [redacted-path]');
	  });
});
