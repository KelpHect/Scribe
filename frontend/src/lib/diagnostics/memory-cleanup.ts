import type { DiagnosticsSnapshot } from '$lib/services/diagnostics-service';

export const memoryCleanupCooldownMs = 2 * 60 * 1000;

export function liveHeapPressureMb(
  diagnostics: Pick<DiagnosticsSnapshot, 'heapAllocMb' | 'heapInUseMb'>
): number {
  const heapAllocMb = Number.isFinite(diagnostics.heapAllocMb) ? diagnostics.heapAllocMb : 0;
  const heapInUseMb =
    typeof diagnostics.heapInUseMb === 'number' && Number.isFinite(diagnostics.heapInUseMb)
      ? diagnostics.heapInUseMb
      : 0;
  return Math.max(0, heapAllocMb, heapInUseMb);
}

export function shouldRunMemoryCleanup(
  diagnostics: Pick<DiagnosticsSnapshot, 'heapAllocMb' | 'heapInUseMb'>,
  memoryLimitMb: number,
  nowMs: number,
  lastCleanupAtMs: number
): boolean {
  if (memoryLimitMb <= 0) return false;
  if (lastCleanupAtMs > 0 && nowMs - lastCleanupAtMs < memoryCleanupCooldownMs) return false;
  return liveHeapPressureMb(diagnostics) >= memoryLimitMb;
}
