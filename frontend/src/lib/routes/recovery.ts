export type RecoverableRouteError = {
  message: string;
  details: string;
};

export type LazyRouteState<TComponent> = {
  component: TComponent | null;
  loading: boolean;
  error: RecoverableRouteError | null;
};

export function createLazyRouteState<TComponent>(): LazyRouteState<TComponent> {
  return {
    component: null,
    loading: false,
    error: null
  };
}

export function normalizeRecoverableError(error: unknown, fallbackMessage: string): RecoverableRouteError {
  if (error instanceof Error) {
    return {
      message: error.message || fallbackMessage,
      details: error.stack || `${error.name}: ${error.message || fallbackMessage}`
    };
  }

  if (typeof error === 'string') {
    return {
      message: error || fallbackMessage,
      details: error || fallbackMessage
    };
  }

  return {
    message: fallbackMessage,
    details: safeStringify(error) || fallbackMessage
  };
}

export async function loadLazyRoute<TComponent>(
  state: LazyRouteState<TComponent>,
  loader: () => Promise<{ default: TComponent }>,
  fallbackMessage: string
): Promise<TComponent | null> {
  if (state.component) return state.component;
  if (state.loading) return null;

  state.loading = true;
  state.error = null;

  try {
    const mod = await loader();
    state.component = mod.default;
    return state.component;
  } catch (error) {
    state.error = normalizeRecoverableError(error, fallbackMessage);
    return null;
  } finally {
    state.loading = false;
  }
}

export function resetLazyRoute<TComponent>(state: LazyRouteState<TComponent>) {
  state.component = null;
  state.loading = false;
  state.error = null;
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return '';
  }
}
