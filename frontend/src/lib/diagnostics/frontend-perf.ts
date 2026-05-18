type TimingMeta = Record<string, string | number | boolean | null | undefined>;

export type TimingMetricSnapshot = {
  name: string;
  count: number;
  lastMs: number;
  avgMs: number;
  maxMs: number;
  p95Ms: number;
  lastAt: string;
  meta: TimingMeta;
};

export type GaugeMetricSnapshot = {
  name: string;
  updatedAt: string;
  value: number;
  meta: TimingMeta;
};

export type ProgressEventMetricSnapshot = {
  totalEvents: number;
  lastMinuteEvents: number;
  lastEventAt: string;
  lastState: string;
  lastUID: string;
  lastTaskCount: number;
  lastActiveCount: number;
  errorEvents: number;
  droppedEvents: number;
};

export type FrontendPerformanceSnapshot = {
  timings: TimingMetricSnapshot[];
  gauges: GaugeMetricSnapshot[];
  progressEvents: ProgressEventMetricSnapshot;
};

type TimingMetric = {
  name: string;
  samples: number[];
  totalMs: number;
  maxMs: number;
  lastMs: number;
  lastAt: string;
  meta: TimingMeta;
};

const maxSamples = 120;
const timings = new Map<string, TimingMetric>();
const gauges = new Map<string, GaugeMetricSnapshot>();
const progressEventTimes: number[] = [];
let progressTotalEvents = 0;
let progressErrorEvents = 0;
let progressDroppedEvents = 0;
let progressLastEventAt = '';
let progressLastState = '';
let progressLastUID = '';
let progressLastTaskCount = 0;
let progressLastActiveCount = 0;

function nowMs(): number {
  return performance.now();
}

function nowISO(): string {
  return new Date().toISOString();
}

function roundMs(value: number): number {
  return Math.round(value * 100) / 100;
}

function percentile(samples: number[], ratio: number): number {
  if (samples.length === 0) return 0;
  const sorted = [...samples].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.ceil(sorted.length * ratio) - 1);
  return sorted[Math.max(0, index)];
}

export function recordFrontendTiming(name: string, durationMs: number, meta: TimingMeta = {}): void {
  const safeDuration = Math.max(0, durationMs);
  const existing =
    timings.get(name) ??
    ({
      name,
      samples: [],
      totalMs: 0,
      maxMs: 0,
      lastMs: 0,
      lastAt: '',
      meta: {}
    } satisfies TimingMetric);

  existing.samples.push(safeDuration);
  if (existing.samples.length > maxSamples) {
    existing.samples.shift();
  }
  existing.totalMs += safeDuration;
  existing.maxMs = Math.max(existing.maxMs, safeDuration);
  existing.lastMs = safeDuration;
  existing.lastAt = nowISO();
  existing.meta = meta;
  timings.set(name, existing);
}

export function measureFrontendTiming<T>(name: string, run: () => T, meta: (result: T) => TimingMeta): T {
  const start = nowMs();
  const result = run();
  recordFrontendTiming(name, nowMs() - start, meta(result));
  return result;
}

export function recordFrontendGauge(name: string, value: number, meta: TimingMeta = {}): void {
  gauges.set(name, {
    name,
    updatedAt: nowISO(),
    value,
    meta
  });
}

export function recordDownloadProgressEvent(input: {
  uid: string;
  state: string;
  taskCount: number;
  activeCount: number;
  error?: string;
  dropped?: boolean;
}): void {
  const now = Date.now();
  progressTotalEvents++;
  progressEventTimes.push(now);

  const cutoff = now - 60_000;
  while (progressEventTimes.length > 0 && progressEventTimes[0] < cutoff) {
    progressEventTimes.shift();
  }

  progressLastEventAt = nowISO();
  progressLastState = input.state;
  progressLastUID = input.uid;
  progressLastTaskCount = input.taskCount;
  progressLastActiveCount = input.activeCount;
  if (input.error) progressErrorEvents++;
  if (input.dropped) progressDroppedEvents++;
}

export function getFrontendPerformanceSnapshot(): FrontendPerformanceSnapshot {
  return {
    timings: [...timings.values()].map((metric) => ({
      name: metric.name,
      count: metric.samples.length,
      lastMs: roundMs(metric.lastMs),
      avgMs: roundMs(metric.totalMs / Math.max(1, metric.samples.length)),
      maxMs: roundMs(metric.maxMs),
      p95Ms: roundMs(percentile(metric.samples, 0.95)),
      lastAt: metric.lastAt,
      meta: metric.meta
    })),
    gauges: [...gauges.values()],
    progressEvents: {
      totalEvents: progressTotalEvents,
      lastMinuteEvents: progressEventTimes.length,
      lastEventAt: progressLastEventAt,
      lastState: progressLastState,
      lastUID: progressLastUID,
      lastTaskCount: progressLastTaskCount,
      lastActiveCount: progressLastActiveCount,
      errorEvents: progressErrorEvents,
      droppedEvents: progressDroppedEvents
    }
  };
}

export function resetFrontendPerformanceDiagnostics(): void {
  timings.clear();
  gauges.clear();
  progressEventTimes.length = 0;
  progressTotalEvents = 0;
  progressErrorEvents = 0;
  progressDroppedEvents = 0;
  progressLastEventAt = '';
  progressLastState = '';
  progressLastUID = '';
  progressLastTaskCount = 0;
  progressLastActiveCount = 0;
}
