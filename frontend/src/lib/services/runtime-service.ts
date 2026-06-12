type RuntimeEvent = { data: unknown };

type WailsRuntimeModule = {
  Application: {
    Quit: () => Promise<void>;
  };
  Browser: {
    OpenURL: (url: string) => Promise<void>;
  };
  Clipboard: {
    Text: () => Promise<string>;
    SetText: (text: string) => Promise<void>;
  };
  Events: {
    Emit: (eventName: string, data: unknown) => Promise<boolean>;
    On: (eventName: string, callback: (event: RuntimeEvent | unknown) => void) => () => void;
  };
  Window: {
    Minimise: () => Promise<void>;
    ToggleMaximise: () => Promise<void>;
  };
};

type RuntimeCompat = {
  EventsEmit: (eventName: string, ...args: any[]) => Promise<boolean>;
  EventsOn: (eventName: string, callback: (data: any) => void) => () => void;
  WindowMinimise: () => Promise<void>;
  WindowToggleMaximise: () => Promise<void>;
  Quit: () => Promise<void>;
  BrowserOpenURL: (url: string) => Promise<void>;
  ClipboardGetText: () => Promise<string>;
  ClipboardSetText: (text: string) => Promise<boolean>;
};

function eventPayload(args: any[]): any {
  if (args.length === 0) return null;
  if (args.length === 1) return args[0];
  return args;
}

function unwrapEventData(event: RuntimeEvent | unknown): unknown {
  if (event && typeof event === 'object' && 'data' in event) {
    return (event as RuntimeEvent).data;
  }
  return event;
}

let runtimeCompatPromise: Promise<RuntimeCompat> | null = null;

function createRuntimeCompat({
  Application,
  Browser,
  Clipboard,
  Events,
  Window
}: WailsRuntimeModule): RuntimeCompat {
  return {
    EventsEmit(eventName, ...args) {
      return Events.Emit(eventName, eventPayload(args));
    },
    EventsOn(eventName, callback) {
      return Events.On(eventName, (event) => callback(unwrapEventData(event)));
    },
    WindowMinimise() {
      return Window.Minimise();
    },
    WindowToggleMaximise() {
      return Window.ToggleMaximise();
    },
    Quit() {
      return Application.Quit();
    },
    BrowserOpenURL(url) {
      return Browser.OpenURL(url);
    },
    ClipboardGetText() {
      return Clipboard.Text();
    },
    async ClipboardSetText(text) {
      await Clipboard.SetText(text);
      return true;
    }
  };
}

async function getRuntime(): Promise<RuntimeCompat> {
  runtimeCompatPromise ??= import('@wailsio/runtime').then((runtime) =>
    createRuntimeCompat(runtime as WailsRuntimeModule)
  );
  return runtimeCompatPromise;
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
