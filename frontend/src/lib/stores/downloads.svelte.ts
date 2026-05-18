import { refreshInstalledState } from '$lib/db/query-state';
import { fetchMissingDependencies } from '$lib/services/esoui-service';
import { getRuntime } from '$lib/services/runtime-service';
import { toast } from 'svelte-sonner';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import { filterNewInstallUIDs, filterRetryInstallUIDs } from '$lib/stores/install-queue';
import { recordDownloadProgressEvent } from '$lib/diagnostics/frontend-perf';

export type TaskState =
  | 'queued'
  | 'planning'
  | 'downloading'
  | 'extracting'
  | 'complete'
  | 'failed'
  | 'cancelled';

export interface InstallPlanEntry {
  folderName: string;
  action: 'add' | 'replace';
  reason: string;
}

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
  installPlan?: InstallPlanEntry[];
}

async function getApp(): Promise<any> {
  return import('wailsjs/go/main/App');
}

const tasks = new SvelteMap<string, TaskProgress>();
const pendingInstallUIDs = new SvelteSet<string>();
let listenerActive: boolean = $state(false);

function getTaskList(): TaskProgress[] {
  return Array.from(tasks.values());
}

function getActiveDownloads(): TaskProgress[] {
  return getTaskList().filter(
    (t) =>
      t.state === 'downloading' ||
      t.state === 'planning' ||
      t.state === 'extracting' ||
      t.state === 'queued'
  );
}

function getCompletedDownloads(): TaskProgress[] {
  return getTaskList().filter((t) => t.state === 'complete');
}

function getFailedDownloads(): TaskProgress[] {
  return getTaskList().filter((t) => t.state === 'failed');
}

function getRecentDownloads(): TaskProgress[] {
  return getTaskList().filter(
    (t) => t.state === 'complete' || t.state === 'failed' || t.state === 'cancelled'
  );
}

function getRetryableFailedDownloads(): TaskProgress[] {
  const retryUIDs = new Set(filterRetryInstallUIDs(getFailedDownloads(), isInstallActive));
  return getFailedDownloads().filter((task) => retryUIDs.has(task.uid));
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

export function isInstallActive(uid: string): boolean {
  if (pendingInstallUIDs.has(uid)) return true;
  const task = tasks.get(uid);
  return (
    task?.state === 'queued' ||
    task?.state === 'downloading' ||
    task?.state === 'planning' ||
    task?.state === 'extracting'
  );
}

function uniquePendingUIDs(uids: string[]): string[] {
  return filterNewInstallUIDs(uids, isInstallActive);
}

let cleanupFn: (() => void) | null = null;

let depCheckTimer: ReturnType<typeof setTimeout> | null = null;

function clearDepCheckTimer() {
  if (depCheckTimer !== null) {
    clearTimeout(depCheckTimer);
    depCheckTimer = null;
  }
}

function scheduleMissingDepCheck() {
  clearDepCheckTimer();
  depCheckTimer = setTimeout(() => {
    depCheckTimer = null;

    if (getActiveCount() > 0) return;
    void runMissingDepCheck();
  }, 1500);
}

let invalidateTimer: ReturnType<typeof setTimeout> | null = null;
function clearInvalidateTimer() {
  if (invalidateTimer !== null) {
    clearTimeout(invalidateTimer);
    invalidateTimer = null;
  }
}

function scheduleInvalidate() {
  clearInvalidateTimer();
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
    recordDownloadProgressEvent({
      uid: data.uid,
      state: data.state,
      taskCount: tasks.size,
      activeCount: getActiveCount(),
      error: data.error
    });

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
  clearDepCheckTimer();
  clearInvalidateTimer();
}

export async function installAddon(uid: string, name?: string): Promise<boolean> {
  const normalizedUID = uid.trim();
  if (!normalizedUID || isInstallActive(normalizedUID)) return false;
  pendingInstallUIDs.add(normalizedUID);
  tasks.set(normalizedUID, {
    uid: normalizedUID,
    name: name ?? normalizedUID,
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
    await app.InstallAddon(normalizedUID);
    return true;
  } catch (e) {
    tasks.delete(normalizedUID);
    throw e;
  } finally {
    pendingInstallUIDs.delete(normalizedUID);
  }
}

export async function batchInstall(
  uids: string[],
  names?: Record<string, string>
): Promise<number> {
  const uniqueUIDs = uniquePendingUIDs(uids);
  if (uniqueUIDs.length === 0) return 0;

  for (const uid of uniqueUIDs) {
    pendingInstallUIDs.add(uid);
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
    return (await app.BatchInstall(uniqueUIDs)) as number;
  } catch (e) {
    for (const uid of uniqueUIDs) {
      if (tasks.get(uid)?.state === 'queued') {
        tasks.delete(uid);
      }
    }
    throw e;
  } finally {
    for (const uid of uniqueUIDs) {
      pendingInstallUIDs.delete(uid);
    }
  }
}

export async function retryFailedInstalls(): Promise<number> {
  const failed = getRetryableFailedDownloads();
  if (failed.length === 0) return 0;

  const names = Object.fromEntries(failed.map((task) => [task.uid, task.name || task.uid]));
  const count = await batchInstall(
    failed.map((task) => task.uid),
    names
  );

  if (count === 0) {
    toast.info('No failed installs to retry');
  }
  return count;
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
    get recentDownloads() {
      return getRecentDownloads();
    },
    get retryableFailedDownloads() {
      return getRetryableFailedDownloads();
    },
    get activeCount() {
      return getActiveCount();
    },
    get isDownloading() {
      return isDownloading();
    },
    getTask,
    isInstallActive,
    installAddon,
    batchInstall,
    retryFailedInstalls,
    cancelInstall,
    cancelAllInstalls,
    clearFinished,
    clearTask,
    startListening,
    stopListening
  };
}
