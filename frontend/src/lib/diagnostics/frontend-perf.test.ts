import { beforeEach, describe, expect, it } from 'vitest';
import {
  getFrontendPerformanceSnapshot,
  recordDownloadProgressEvent,
  recordFrontendGauge,
  recordFrontendTiming,
  resetFrontendPerformanceDiagnostics
} from './frontend-perf';

describe('frontend performance diagnostics', () => {
  beforeEach(() => {
    resetFrontendPerformanceDiagnostics();
  });

  it('records timing samples with rounded aggregates and metadata', () => {
    recordFrontendTiming('findMore.filterSort', 2.25, { resultCount: 10 });
    recordFrontendTiming('findMore.filterSort', 4.75, { resultCount: 20 });

    const snapshot = getFrontendPerformanceSnapshot();
    expect(snapshot.timings).toEqual([
      expect.objectContaining({
        name: 'findMore.filterSort',
        count: 2,
        lastMs: 4.75,
        avgMs: 3.5,
        maxMs: 4.75,
        p95Ms: 4.75,
        meta: { resultCount: 20 }
      })
    ]);
  });

  it('records gauges and download progress event rates', () => {
    recordFrontendGauge('findMore.visibleItems', 12, { resultCount: 100 });
    recordDownloadProgressEvent({
      uid: '101',
      state: 'downloading',
      taskCount: 2,
      activeCount: 1
    });
    recordDownloadProgressEvent({
      uid: '101',
      state: 'failed',
      taskCount: 2,
      activeCount: 0,
      error: 'network'
    });

    const snapshot = getFrontendPerformanceSnapshot();
    expect(snapshot.gauges).toEqual([
      expect.objectContaining({
        name: 'findMore.visibleItems',
        value: 12,
        meta: { resultCount: 100 }
      })
    ]);
    expect(snapshot.progressEvents).toEqual(
      expect.objectContaining({
        totalEvents: 2,
        lastMinuteEvents: 2,
        lastState: 'failed',
        lastUID: '101',
        lastTaskCount: 2,
        lastActiveCount: 0,
        errorEvents: 1,
        droppedEvents: 0
      })
    );
  });
});
