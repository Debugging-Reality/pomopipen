import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../', import.meta.url));
export default defineConfig({
  root: fileURLToPath(new URL('.', import.meta.url)),
  publicDir: `${root}/static`,
  plugins: [svelte()],
  resolve: { alias: [
    { find: '$lib/ipc', replacement: `${root}/tests/preview/mock-ipc.ts` },
    { find: /^@tauri-apps\/(api\/webviewWindow|api\/webview|plugin-log)$/, replacement: `${root}/tests/preview/mock-tauri.ts` },
    { find: '$lib', replacement: `${root}/src/lib` },
    { find: '$paraglide', replacement: `${root}/src/paraglide` },
  ] },
  server: { host: '127.0.0.1', port: 1422, strictPort: true, fs: { allow: [root] } },
});
