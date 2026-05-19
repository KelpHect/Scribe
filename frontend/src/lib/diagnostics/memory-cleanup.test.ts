import { describe, expect, it } from 'vitest';
import {
  liveHeapPressureMb,
  memoryCleanupCooldownMs,
  shouldRunMemoryCleanup
} from './memory-cleanup';

describe('memory cleanup decisions', () => {
  it('uses live heap pressure instead of reserved runtime memory', () => {
    expect(liveHeapPressureMb({ heapAllocMb: 42, heapInUseMb: 55 })).toBe(55);
  });

  it('does not trigger cleanup just because Go sys memory stayed high', () => {
    expect(
      shouldRunMemoryCleanup(
        {
          heapAllocMb: 40,
          heapInUseMb: 50
        },
        150,
        10_000,
        0
      )
    ).toBe(false);
  });

  it('triggers on heap pressure and respects the cooldown', () => {
    expect(shouldRunMemoryCleanup({ heapAllocMb: 175, heapInUseMb: 170 }, 150, 10_000, 0)).toBe(
      true
    );
    expect(
      shouldRunMemoryCleanup(
        { heapAllocMb: 175, heapInUseMb: 170 },
        150,
        10_000 + memoryCleanupCooldownMs - 1,
        10_000
      )
    ).toBe(false);
    expect(
      shouldRunMemoryCleanup(
        { heapAllocMb: 175, heapInUseMb: 170 },
        150,
        10_000 + memoryCleanupCooldownMs,
        10_000
      )
    ).toBe(true);
  });
});
