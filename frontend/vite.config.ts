import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';
import { defineConfig } from 'vite';
import buildReportPlugin from './vite-plugin-build-report.js';

export default defineConfig({
  plugins: [svelte(), tailwindcss(), buildReportPlugin()],
  build: {
    target: 'esnext',
    reportCompressedSize: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          const normalized = id.replace(/\\/g, '/');

          if (normalized.includes('/node_modules/svelte/')) {
            return 'vendor-svelte';
          }

          if (normalized.includes('/node_modules/lucide-svelte/')) {
            return 'vendor-icons';
          }

          if (
            normalized.includes('/node_modules/@tanstack/query-core/') ||
            normalized.includes('/node_modules/@tanstack/svelte-query/')
          ) {
            return 'vendor-tanstack-data';
          }

          if (
            normalized.includes('/node_modules/@tanstack/svelte-virtual/') ||
            normalized.includes('/node_modules/@tanstack/svelte-form/')
          ) {
            return 'vendor-tanstack-ui';
          }

          if (
            normalized.includes('/node_modules/bits-ui/') ||
            normalized.includes('/node_modules/svelte-sonner/') ||
            normalized.includes('/node_modules/valibot/')
          ) {
            return 'vendor-ui';
          }

          if (
            normalized.includes('/src/routes/InstalledPage.svelte') ||
            normalized.includes('/src/lib/components/addon/AddonCard.svelte') ||
            normalized.includes('/src/lib/components/addon/AddonDetail.svelte')
          ) {
            return 'route-installed';
          }

          if (
            normalized.includes('/src/routes/FindMorePage.svelte') ||
            normalized.includes('/src/lib/components/addon/RemoteAddonDetail.svelte')
          ) {
            return 'route-find-more';
          }

          if (normalized.includes('/src/routes/UpdatesPage.svelte')) {
            return 'route-updates';
          }

          if (normalized.includes('/src/routes/SettingsPage.svelte')) {
            return 'route-settings';
          }
        }
      }
    }
  },
  resolve: {
    alias: {
      $lib: path.resolve('./src/lib'),
      wailsjs: path.resolve('./wailsjs')
    }
  }
});
