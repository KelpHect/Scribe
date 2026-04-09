<svelte:options runes />

<script lang="ts">
  import { onMount } from 'svelte';
  import { Toaster } from 'svelte-sonner';
  import { QueryClientProvider } from '@tanstack/svelte-query';
  import Copy from 'lucide-svelte/icons/copy';
  import ExternalLink from 'lucide-svelte/icons/external-link';
  import ClipboardPaste from 'lucide-svelte/icons/clipboard-paste';
  import Scissors from 'lucide-svelte/icons/scissors';
  import WholeWord from 'lucide-svelte/icons/whole-word';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import { DownloadQueue } from '$lib/components/download';
  import type { ContextMenuEntry } from '$lib/services/context-menu-service';
  import { fetchDiagnostics, performMemoryCleanup } from '$lib/services/diagnostics-service';
  import {
    clipboardGetText,
    clipboardSetText,
    emitRuntimeEvent,
    openExternalURL
  } from '$lib/services/runtime-service';
  import { applyTheme } from '$lib/services/theme-service';
  import { getSettings } from '$lib/services/settings-service';
  import { navigation, getDownloadStore } from '$lib/stores';
  import { queryClient } from '$lib/db/client';
  import type { Page } from '$lib/stores/navigation.svelte';

  import InstalledPage from './routes/InstalledPage.svelte';

  let FindMorePage = $state<any>(null);
  let UpdatesPage = $state<any>(null);
  let SettingsPage = $state<any>(null);

  async function preloadPage(page: Page) {
    switch (page) {
      case 'find-more':
        if (!FindMorePage) {
          const mod = await import('./routes/FindMorePage.svelte');
          FindMorePage = mod.default;
        }
        break;
      case 'updates':
        if (!UpdatesPage) {
          const mod = await import('./routes/UpdatesPage.svelte');
          UpdatesPage = mod.default;
        }
        break;
      case 'settings':
        if (!SettingsPage) {
          const mod = await import('./routes/SettingsPage.svelte');
          SettingsPage = mod.default;
        }
        break;
    }
  }

  $effect(() => {
    const page = navigation.current;
    if (page !== 'installed') preloadPage(page);
  });

  const downloads = getDownloadStore();
  let contextMenuOpen = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let contextMenuItems = $state<ContextMenuEntry[]>([]);
  let memoryLimitMb = $state(150);
  let memoryCleanupRunning = false;

  function closeContextMenu() {
    contextMenuOpen = false;
    contextMenuItems = [];
  }

  function getTextInputTarget(target: EventTarget | null): HTMLInputElement | HTMLTextAreaElement | null {
    const node = target instanceof HTMLElement ? target.closest('input, textarea') : null;
    if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) return node;
    return null;
  }

  function getInputSelection(input: HTMLInputElement | HTMLTextAreaElement) {
    const start = input.selectionStart ?? 0;
    const end = input.selectionEnd ?? 0;
    return { start, end, text: start !== end ? input.value.slice(start, end) : '' };
  }

  function dispatchTextInputEvents(input: HTMLInputElement | HTMLTextAreaElement) {
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }

  async function loadMemoryLimit() {
    try {
      const settings = await getSettings();
      memoryLimitMb = settings.memoryLimitMb ?? 150;
      applyTheme(settings.theme);
    } catch {
      memoryLimitMb = 150;
      applyTheme('scribe');
    }
  }

  async function maybeCleanupMemory() {
    if (memoryCleanupRunning || memoryLimitMb <= 0) return;
    const diagnostics = await fetchDiagnostics();
    const currentMb = Math.max(diagnostics.heapAllocMb, diagnostics.sysMb);
    if (currentMb < memoryLimitMb) return;

    memoryCleanupRunning = true;
    try {
      queryClient.removeQueries({ queryKey: ['addon-details'] });
      await performMemoryCleanup();
      void emitRuntimeEvent('perf:capture', 'memory-cleanup').catch(() => undefined);
    } finally {
      memoryCleanupRunning = false;
    }
  }

  onMount(() => {
    downloads.startListening();
    navigation.setPreload(preloadPage);
    void loadMemoryLimit();
    const handleGlobalKeydown = (e: KeyboardEvent) => {
      const key = e.key.toLowerCase();
      const mod = e.ctrlKey || e.metaKey;

      if (mod && key === '1') {
        e.preventDefault();
        navigation.navigate('installed');
        return;
      }

      if (mod && key === '2') {
        e.preventDefault();
        navigation.navigate('find-more');
        return;
      }

      if (mod && key === 'u') {
        e.preventDefault();
        navigation.navigate('updates');
        return;
      }

      if (mod && key === 'f') {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('scribe:focus-search'));
        return;
      }

      if (key === 'escape') {
        closeContextMenu();
        window.dispatchEvent(new CustomEvent('scribe:close-modal'));
      }
    };

    const handleContextMenu = (e: MouseEvent) => {
      if (e.defaultPrevented) return;
      const input = getTextInputTarget(e.target);
      const link = e.target instanceof HTMLElement ? (e.target.closest('a[href]') as HTMLAnchorElement | null) : null;
      const selectionText = input
        ? getInputSelection(input).text
        : window.getSelection?.()?.toString().trim() || '';

      const items: ContextMenuEntry[] = [];

      if (link?.href) {
        items.push({ label: 'Open Link', icon: ExternalLink, action: () => openExternalURL(link.href) });
        items.push({ type: 'separator' });
        items.push({
          label: 'Copy Link',
          icon: Copy,
          action: async () => {
            await clipboardSetText(link.href);
          }
        });
      }

      if (input) {
        const getSelection = () => getInputSelection(input);
        if (getSelection().text) {
          items.push({
            label: 'Copy',
            icon: Copy,
            action: async () => {
              await clipboardSetText(getSelection().text);
            }
          });
          items.push({
            label: 'Cut',
            icon: Scissors,
            action: async () => {
              const { start, end, text } = getSelection();
              if (!text) return;
              await clipboardSetText(text);
              input.setRangeText('', start, end, 'start');
              dispatchTextInputEvents(input);
            }
          });
          items.push({ type: 'separator' });
        }
        items.push({
          label: 'Paste',
          icon: ClipboardPaste,
          action: async () => {
            const text = await clipboardGetText();
            if (!text) return;
            const { start, end } = getSelection();
            input.setRangeText(text, start, end, 'end');
            dispatchTextInputEvents(input);
          }
        });
        items.push({
          label: 'Select All',
          icon: WholeWord,
          action: () => {
            input.focus();
            input.select();
          }
        });
      } else if (selectionText) {
        items.push({
          label: 'Copy',
          icon: Copy,
          action: async () => {
            await clipboardSetText(selectionText);
          }
        });
      }

      if (items.length === 0) {
        items.push({ label: 'No actions available', action: () => undefined, disabled: true });
      }

      e.preventDefault();
      contextMenuX = e.clientX;
      contextMenuY = e.clientY;
      contextMenuItems = items;
      contextMenuOpen = true;
    };

    const handleCustomContextMenu = (e: Event) => {
      const detail = (e as CustomEvent<{ x: number; y: number; items: ContextMenuEntry[] }>).detail;
      if (!detail) return;
      contextMenuX = detail.x;
      contextMenuY = detail.y;
      contextMenuItems = detail.items;
      contextMenuOpen = true;
    };

    const handleSettingsUpdated = (e: Event) => {
      const detail = (e as CustomEvent<{ memoryLimitMb?: number; theme?: string }>).detail;
      if (typeof detail?.memoryLimitMb === 'number') {
        memoryLimitMb = detail.memoryLimitMb;
      }
      if (detail?.theme) {
        applyTheme(detail.theme);
      }
    };

    const closeMenu = () => closeContextMenu();

    window.addEventListener('keydown', handleGlobalKeydown);
    window.addEventListener('contextmenu', handleContextMenu);
    window.addEventListener('scribe:open-context-menu', handleCustomContextMenu as EventListener);
    window.addEventListener('scribe:settings-updated', handleSettingsUpdated as EventListener);
    window.addEventListener('resize', closeMenu);
    window.addEventListener('scroll', closeMenu, true);
    window.addEventListener('blur', closeMenu);

    let readyFrame1 = 0;
    let readyFrame2 = 0;
    readyFrame1 = window.requestAnimationFrame(() => {
      readyFrame2 = window.requestAnimationFrame(() => {
        void emitRuntimeEvent('perf:frontend-ready').catch(() => undefined);
      });
    });

    const idleTimer = window.setTimeout(() => {
      void emitRuntimeEvent('perf:capture', 'idle-after-startup').catch(() => undefined);
    }, 2000);
    const memoryTimer = window.setInterval(() => {
      void maybeCleanupMemory();
    }, 30000);

    return () => {
      window.cancelAnimationFrame(readyFrame1);
      window.cancelAnimationFrame(readyFrame2);
      window.clearTimeout(idleTimer);
      window.clearInterval(memoryTimer);
      window.removeEventListener('keydown', handleGlobalKeydown);
      window.removeEventListener('contextmenu', handleContextMenu);
      window.removeEventListener('scribe:open-context-menu', handleCustomContextMenu as EventListener);
      window.removeEventListener('scribe:settings-updated', handleSettingsUpdated as EventListener);
      window.removeEventListener('resize', closeMenu);
      window.removeEventListener('scroll', closeMenu, true);
      window.removeEventListener('blur', closeMenu);
      downloads.stopListening();
    };
  });
</script>

<QueryClientProvider client={queryClient}>
  <Toaster richColors position="bottom-right" />
  <TitleBar />

  <div class="flex flex-1 overflow-hidden">
    <Sidebar />
    <main class="flex-1 overflow-hidden">
      <div class="h-full" class:hidden={navigation.current !== 'installed'}>
        <InstalledPage />
      </div>
      {#if FindMorePage}
        <div class="h-full" class:hidden={navigation.current !== 'find-more'}>
          <FindMorePage />
        </div>
      {/if}
      {#if UpdatesPage}
        <div class="h-full" class:hidden={navigation.current !== 'updates'}>
          <UpdatesPage />
        </div>
      {/if}
      {#if SettingsPage}
        <div class="h-full" class:hidden={navigation.current !== 'settings'}>
          <SettingsPage />
        </div>
      {/if}
    </main>
  </div>

  <DownloadQueue />
  <ContextMenu
    open={contextMenuOpen}
    x={contextMenuX}
    y={contextMenuY}
    items={contextMenuItems}
    onclose={closeContextMenu}
  />
</QueryClientProvider>
