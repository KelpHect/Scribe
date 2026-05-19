import { callWails } from '$lib/services/wails-service';

export interface AppInfo {
  version: string;
  commit: string;
  buildDate: string;
  goVersion: string;
  os: string;
  arch: string;
  customTitleBar: boolean;
}

export async function fetchAppInfo(): Promise<AppInfo> {
  const info = (await callWails('GetAppInfo')) as AppInfo | undefined;
  return {
    version: info?.version ?? 'unknown',
    commit: info?.commit ?? 'unknown',
    buildDate: info?.buildDate ?? 'unknown',
    goVersion: info?.goVersion ?? 'unknown',
    os: info?.os ?? 'unknown',
    arch: info?.arch ?? 'unknown',
    customTitleBar: info?.customTitleBar ?? info?.os !== 'linux'
  };
}
