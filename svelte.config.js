import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      // Tauri serves from a local file path, not a server
      fallback: 'index.html',
    }),
    alias: {
      $lib: 'src/lib',
    },
  },
};

export default config;
