import { callWails } from '$lib/services/wails-service';

export interface AppInfo {
  version: string;
  commit: string;
  buildDate: string;
  goVersion: string;
  os: string;
  arch: string;
}

export async function fetchAppInfo(): Promise<AppInfo> {
  return await callWails('GetAppInfo');
}
