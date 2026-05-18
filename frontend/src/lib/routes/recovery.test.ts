import { describe, expect, it } from 'vitest';
import { createLazyRouteState, loadLazyRoute, normalizeRecoverableError } from './recovery';

describe('normalizeRecoverableError', () => {
  it('keeps failed service messages copyable', () => {
    const error = normalizeRecoverableError(new Error('Wails service unavailable'), 'Service failed');

    expect(error.message).toBe('Wails service unavailable');
    expect(error.details).toContain('Wails service unavailable');
  });
});

describe('loadLazyRoute', () => {
  it('records failed dynamic route imports and can retry successfully', async () => {
    const state = createLazyRouteState<string>();
    let attempts = 0;

    const first = await loadLazyRoute(
      state,
      async () => {
        attempts += 1;
        throw new Error('route chunk missing');
      },
      'Failed to load route'
    );

    expect(first).toBeNull();
    expect(state.component).toBeNull();
    expect(state.loading).toBe(false);
    expect(state.error?.message).toBe('route chunk missing');

    const second = await loadLazyRoute(
      state,
      async () => {
        attempts += 1;
        return { default: 'RecoveredPage' };
      },
      'Failed to load route'
    );

    expect(second).toBe('RecoveredPage');
    expect(state.component).toBe('RecoveredPage');
    expect(state.error).toBeNull();
    expect(attempts).toBe(2);
  });

  it('keeps route states independent so mounted pages can preserve state', async () => {
    const installed = createLazyRouteState<string>();
    const findMore = createLazyRouteState<string>();

    await loadLazyRoute(
      installed,
      async () => ({ default: 'InstalledPage' }),
      'Failed to load installed route'
    );
    await loadLazyRoute(
      findMore,
      async () => {
        throw new Error('find more chunk missing');
      },
      'Failed to load find more route'
    );

    expect(installed.component).toBe('InstalledPage');
    expect(installed.error).toBeNull();
    expect(findMore.component).toBeNull();
    expect(findMore.error?.message).toBe('find more chunk missing');
  });
});
