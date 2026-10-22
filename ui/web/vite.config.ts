import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'node:fs';

const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as { version: string };

// 产物为纯静态文件，由桌面宿主（CEF 自定义域 / 开发 HTTP 宿主）加载，完全离线。
export default defineConfig({
  base: './',
  plugins: [svelte()],
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'chrome120',
    assetsInlineLimit: 0,
  },
  server: {
    // `npm run dev` 时把桥接请求转发给 `es-desktop serve`
    proxy: { '/__es': 'http://127.0.0.1:7450' },
  },
});
