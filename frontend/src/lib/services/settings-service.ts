import { normalizeTheme, type AppTheme } from '$lib/services/theme-service';
import { callWails } from '$lib/services/wails-service';

export interface AppSettings {
  addonPath: string;
  autoUpdate: boolean;
  memoryLimitMb: number;
  theme: AppTheme;
}

export async function getSettings(): Promise<AppSettings> {
  try {
    const settings = await callWails('GetSettings');
      return {
        addonPath: settings.addonPath,
        autoUpdate: settings.autoUpdate,
        memoryLimitMb: settings.memoryLimitMb ?? 150,
        theme: normalizeTheme(settings.theme)
      };
  } catch {
    return {
      addonPath: '',
      autoUpdate: false,
      memoryLimitMb: 150,
      theme: 'scribe'
    };
  }
}

export async function saveSettings(s: AppSettings): Promise<void> {
  await callWails('SaveSettings', s);
}
