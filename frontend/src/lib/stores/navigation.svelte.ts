export type Page = 'installed' | 'find-more' | 'updates' | 'settings';

class NavigationStore {
  current: Page = $state<Page>('installed');
  pendingSearch: string = $state('');
  preloadFn: ((page: Page) => void) | null = null;

  navigate(page: Page, search?: string): void {
    this.pendingSearch = search ?? '';
    this.current = page;
  }

  isCurrent(page: Page): boolean {
    return this.current === page;
  }

  setPreload(fn: (page: Page) => void): void {
    this.preloadFn = fn;
  }

  preload(page: Page): void {
    if (this.preloadFn) this.preloadFn(page);
  }
}

export const navigation = new NavigationStore();
