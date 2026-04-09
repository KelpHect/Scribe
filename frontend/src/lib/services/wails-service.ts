type AppBindings = typeof import('../../../wailsjs/go/main/App');

export async function getWailsApp(): Promise<AppBindings> {
  return import('wailsjs/go/main/App') as Promise<AppBindings>;
}

export async function callWails<TMethod extends keyof AppBindings>(
  method: TMethod,
  ...args: Parameters<AppBindings[TMethod]>
): Promise<Awaited<ReturnType<AppBindings[TMethod]>>> {
  const app = await getWailsApp();
  const fn = app[method] as (
    ..._callArgs: Parameters<AppBindings[TMethod]>
  ) => ReturnType<AppBindings[TMethod]>;
  return (await fn(...args)) as Awaited<ReturnType<AppBindings[TMethod]>>;
}
