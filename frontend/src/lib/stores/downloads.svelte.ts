import { refreshInstalledState } from '$lib/db/query-state';
import { fetchMissingDependencies } from '$lib/services/esoui-service';
import { getRuntime } from '$lib/services/runtime-service';
import { toast } from 'svelte-sonner';
import { SvelteMap } from 'svelte/reactivity';

export type TaskState =
  | 'queued'
  | 'downloading'
  | 'extracting'
  | 'complete'
  | 'failed'
  | 'cancelled';

export interface TaskProgress {
  uid: string;
  name: string;
  state: TaskState;
  percent: number;
  bytesDownloaded: number;
  totalBytes: number;
  speed: number;
  error?: string;
  filesExtracted: number;
  totalFiles: number;
  queuePosition: number;
}

async function getApp(): Promise<any> {
  return import('wailsjs/go/main/App');
}

const tasks = new SvelteMap<string, TaskProgress>();
let listenerActive: boolean = $state(false);

function getTaskList(): TaskProgress[] {
  return Array.from(tasks.values());
}

function getActiveDownloads(): TaskProgress[] {
  return getTaskList().filter(
    (t) => t.state === 'downloading' || t.state === 'extracting' || t.state === 'queued'
  );
}

function getCompletedDownloads(): TaskProgress[] {
  return getTaskList().filter((t) => t.state === 'complete');
}

function getFailedDownloads(): TaskProgress[] {
  return getTaskList().filter((t) => t.state === 'failed');
}

function getActiveCount(): number {
  return getActiveDownloads().length;
}

function isDownloading(): boolean {
  return getActiveCount() > 0;
}

function getTask(uid: string): TaskProgress | undefined {
  return tasks.get(uid);
}

let cleanupFn: (() => void) | null = null;

let depCheckTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleMissingDepCheck() {
  if (depCheckTimer !== null) clearTimeout(depCheckTimer);
  depCheckTimer = setTimeout(() => {
    depCheckTimer = null;

    if (getActiveCount() > 0) return;
    void runMissingDepCheck();
  }, 1500);
}

let invalidateTimer: ReturnType<typeof setTimeout> | null = null;
function scheduleInvalidate() {
  if (invalidateTimer !== null) clearTimeout(invalidateTimer);
  invalidateTimer = setTimeout(() => {
    invalidateTimer = null;
    void refreshInstalledState();
  }, 500);
}

async function runMissingDepCheck() {
  const missing = await fetchMissingDependencies();
  const installable = missing.filter((d) => d.canInstall && !d.optional);
  if (installable.length === 0) return;

  const n = installable.length;
  const label = n === 1 ? `1 missing dependency` : `${n} missing dependencies`;

  toast.warning(`${label} detected`, {
    description:
      installable
        .map((d) => d.remoteName || d.depFolderName)
        .slice(0, 3)
        .join(', ') + (installable.length > 3 ? ` +${installable.length - 3} more` : ''),
    action: {
      label: 'Install All',
      onClick: () => {
        const uids = installable.map((d) => d.remoteUID);
        void batchInstall(uids);
      }
    },
    duration: 12000
  });
}

export async function startListening(): Promise<void> {
  if (listenerActive) return;
  const runtime = await getRuntime().catch(() => null);
  if (!runtime?.EventsOn) return;

  const unsubscribe = runtime.EventsOn('download:progress', (data: TaskProgress) => {
    const prev = tasks.get(data.uid);
    tasks.set(data.uid, data);

    const prevState = prev?.state;
    if (data.state !== prevState) {
      if (data.state === 'complete') {
        toast.success(`${data.name || data.uid} installed successfully`);
        scheduleInvalidate();

        scheduleMissingDepCheck();
      } else if (data.state === 'failed') {
        toast.error(`${data.name || data.uid} failed`, {
          description: data.error || 'Unknown error'
        });
      }
    }
  });

  listenerActive = true;

  cleanupFn = () => {
    unsubscribe?.();
    listenerActive = false;
    cleanupFn = null;
  };
}

export function stopListening(): void {
  if (cleanupFn) cleanupFn();
}

export async function installAddon(uid: string, name?: string): Promise<void> {
  tasks.set(uid, {
    uid,
    name: name ?? uid,
    state: 'queued',
    percent: 0,
    bytesDownloaded: 0,
    totalBytes: 0,
    speed: 0,
    filesExtracted: 0,
    totalFiles: 0,
    queuePosition: 0
  });
  try {
    const app = await getApp();
    await app.InstallAddon(uid);
  } catch (e) {
    tasks.delete(uid);
    throw e;
  }
}

export async function batchInstall(
  uids: string[],
  names?: Record<string, string>
): Promise<number> {
  for (const uid of uids) {
    tasks.set(uid, {
      uid,
      name: names?.[uid] ?? uid,
      state: 'queued',
      percent: 0,
      bytesDownloaded: 0,
      totalBytes: 0,
      speed: 0,
      filesExtracted: 0,
      totalFiles: 0,
      queuePosition: 0
    });
  }
  try {
    const app = await getApp();
    return (await app.BatchInstall(uids)) as number;
  } catch (e) {
    for (const uid of uids) {
      if (tasks.get(uid)?.state === 'queued') {
        tasks.delete(uid);
      }
    }
    throw e;
  }
}

export async function cancelInstall(uid: string): Promise<void> {
  const app = await getApp();
  app.CancelInstall(uid);
}

export async function cancelAllInstalls(): Promise<void> {
  const app = await getApp();
  app.CancelAllInstalls();
}

export async function fetchDownloadQueue(): Promise<TaskProgress[]> {
  try {
    const app = await getApp();
    return ((await app.GetDownloadQueue()) as TaskProgress[]) ?? [];
  } catch {
    return [];
  }
}

export function clearFinished(): void {
  for (const [uid, t] of tasks) {
    if (t.state === 'complete' || t.state === 'failed' || t.state === 'cancelled') {
      tasks.delete(uid);
    }
  }
}

export function clearTask(uid: string): void {
  tasks.delete(uid);
}

export function getDownloadStore() {
  return {
    get tasks() {
      return getTaskList();
    },
    get activeDownloads() {
      return getActiveDownloads();
    },
    get completedDownloads() {
      return getCompletedDownloads();
    },
    get failedDownloads() {
      return getFailedDownloads();
    },
    get activeCount() {
      return getActiveCount();
    },
    get isDownloading() {
      return isDownloading();
    },
    getTask,
    installAddon,
    batchInstall,
    cancelInstall,
    cancelAllInstalls,
    clearFinished,
    clearTask,
    startListening,
    stopListening
  };
}
