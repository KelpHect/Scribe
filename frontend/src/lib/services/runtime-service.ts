async function getRuntime(): Promise<any> {
  return import('wailsjs/runtime/runtime');
}

export async function emitRuntimeEvent(eventName: string, ...args: any[]): Promise<void> {
  const runtime = await getRuntime();
  await runtime.EventsEmit(eventName, ...args);
}

export async function windowMinimise(): Promise<void> {
  const runtime = await getRuntime();
  await runtime.WindowMinimise();
}

export async function windowToggleMaximise(): Promise<void> {
  const runtime = await getRuntime();
  await runtime.WindowToggleMaximise();
}

export async function quitApp(): Promise<void> {
  const runtime = await getRuntime();
  await runtime.Quit();
}

export async function openExternalURL(url: string): Promise<void> {
  const runtime = await getRuntime();
  await runtime.BrowserOpenURL(url);
}

export async function clipboardGetText(): Promise<string> {
  const runtime = await getRuntime();
  return (await runtime.ClipboardGetText()) ?? '';
}

export async function clipboardSetText(text: string): Promise<boolean> {
  const runtime = await getRuntime();
  return !!(await runtime.ClipboardSetText(text));
}

export { getRuntime };
