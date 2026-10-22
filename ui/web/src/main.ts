// 字体全部离线打包（OFL）：Space Grotesk（标题）、Manrope（界面与原文，可变字重以支持夜间 650）、Noto Sans（IPA）。
import '@fontsource-variable/space-grotesk/wght.css';
import '@fontsource-variable/manrope/wght.css';
import '@fontsource/noto-sans/latin-400.css';
import '@fontsource/noto-sans/latin-ext-400.css';
import '@fontsource/noto-sans/latin-500.css';
import '@fontsource/noto-sans/latin-ext-500.css';
import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { store } from './lib/store.svelte';

// 首帧前就定好主题，避免闪烁
document.documentElement.classList.add(store.dark ? 't-dark' : 't-light');

const app = mount(App, { target: document.getElementById('app')! });

// 调试/截图钩子：CDP `Runtime.evaluate` 可直接读取/切换状态
(window as unknown as { es: unknown }).es = { store };

export default app;
