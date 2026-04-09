import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

// leave script preprocessing off b/c svelte 5 already handles ts and `script: true`
// makes oxc strip template-only imports, which explodes stuff like svelte-sonner at runtime
export default {
  preprocess: vitePreprocess()
};
