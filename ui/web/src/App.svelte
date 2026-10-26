<script lang="ts">
  // 主窗口（UI_SPEC §5）：左侧栏 248 · 主区 · 底部悬浮玻璃播放条（相对整个窗口居中）；
  // 点播放条封面展开全屏播放页（原文逐词同步在播放页里）。
  import { fade, fly } from 'svelte/transition';
  import { dur } from './lib/motion';
  import Sidebar from './components/Sidebar.svelte';
  import Home from './components/Home.svelte';
  import ListPage from './components/ListPage.svelte';
  import CollectionPage from './components/CollectionPage.svelte';
  import Settings from './components/Settings.svelte';
  import Player from './components/Player.svelte';
  import PlayerBar from './components/PlayerBar.svelte';
  import ImportDialog from './components/ImportDialog.svelte';
  import ContextMenu from './components/ContextMenu.svelte';
  import WordPopover from './components/WordPopover.svelte';
  import ConfirmDialog from './components/ConfirmDialog.svelte';
  import Titlebar from './components/Titlebar.svelte';
  import Icon from './components/Icon.svelte';
  import { store } from './lib/store.svelte';

  let sidebar = $state<ReturnType<typeof Sidebar>>();
  let main = $state<HTMLElement>();
  let error = $state('');
  let dragging = $state(false);

  store.init().catch((e) => (error = String(e?.message ?? e)));

  // 主题与阅读排版 → 根元素的类与 CSS 变量
  $effect(() => {
    const root = document.documentElement;
    root.classList.toggle('t-dark', store.dark);
    root.classList.toggle('t-light', !store.dark);
    const p = store.prefs;
    const cur = p.fontSize - 1;
    const oth = Math.round((cur * 18) / 23);
    root.style.setProperty('--tx-cur', `${cur}px`);
    root.style.setProperty('--tx-cur-lh', `${Math.round(cur * p.lineHeight)}px`);
    root.style.setProperty('--tx-oth', `${oth}px`);
    root.style.setProperty('--tx-oth-lh', `${Math.round(oth * p.lineHeight)}px`);
    // “非当前句亮度”只改 other 的 alpha；“适中”与设计令牌一致（夜间 .55 / 白天 .65）
    root.style.setProperty('--other-a', String(p.otherLevel / 100 + (store.dark ? -0.05 : 0.05)));
  });

  // 换页时回到顶部
  $effect(() => {
    void store.view;
    main?.scrollTo({ top: 0 });
  });

  function keydown(e: KeyboardEvent) {
    const el = e.target as HTMLElement;
    if (el.closest('input, textarea, select, [contenteditable]')) return;
    if (store.importOpen || store.confirm) return;
    const mod = e.metaKey || e.ctrlKey;
    if (e.key === ' ') {
      if (el.closest('button, [role="button"], a')) return; // 让空格激活聚焦的按钮
      e.preventDefault();
      store.toggle();
    } else if (e.key === 'ArrowLeft' && mod) {
      e.preventDefault();
      store.prevSentence();
    } else if (e.key === 'ArrowRight' && mod) {
      e.preventDefault();
      store.nextSentence();
    } else if (e.key === 'ArrowLeft' && !e.altKey && !e.shiftKey) {
      e.preventDefault();
      store.skip(-5000);
    } else if (e.key === 'ArrowRight' && !e.altKey && !e.shiftKey) {
      e.preventDefault();
      store.skip(5000);
    } else if (!mod && !e.altKey && e.key.toLowerCase() === 'l') {
      e.preventDefault();
      store.toggleLoop();
    } else if (e.key === 'Escape') {
      if (store.popover) store.popover = null;
      else if (store.menu) store.menu = null;
      else if (store.queueOpen) store.queueOpen = false;
      else if (store.playerOpen) store.closePlayer();
    } else if (mod && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      sidebar?.focusSearch();
    } else if (mod && e.key.toLowerCase() === 'o') {
      e.preventDefault();
      store.importOpen = true;
    }
  }

  // 拖入 .txt / .md 到窗口任意位置 → 打开导入对话框并填入文件内容
  async function drop(e: DragEvent) {
    if (!e.dataTransfer?.types.includes('Files')) return;
    e.preventDefault();
    dragging = false;
    const f = e.dataTransfer.files[0];
    if (!f) return;
    if (!/\.(txt|md|markdown|text)$/i.test(f.name)) {
      store.flash('只支持 .txt / .md 文件');
      return;
    }
    store.importFile = { name: f.name, text: await f.text() };
    store.importOpen = false;
    queueMicrotask(() => (store.importOpen = true));
  }
</script>

<svelte:window onkeydown={keydown} />

<div
  class="window"
  class:caption-right={store.host.host === 'cef' && store.host.os !== 'macos'}
  role="application"
  ondragover={(e) => {
    if (e.dataTransfer?.types.includes('Files') && !store.importOpen) {
      e.preventDefault();
      dragging = true;
    }
  }}
  ondragleave={(e) => !e.relatedTarget && (dragging = false)}
  ondrop={drop}
  oncontextmenu={(e) => {
    if (!(e.target as HTMLElement).closest('input, textarea')) e.preventDefault();
  }}
>
  <Titlebar />

  <Sidebar bind:this={sidebar} />

  <main class="main fade-under scroll" bind:this={main}>
    {#if error}
      <div class="fatal">无法连接核心：{error}</div>
    {:else if store.view.kind === 'home'}
      <Home />
    {:else if store.view.kind === 'list'}
      <ListPage filter={store.view.filter} />
    {:else if store.view.kind === 'collection'}
      {@const c = store.collection(store.view.id)}
      {#if c}<CollectionPage {c} />{/if}
    {:else if store.view.kind === 'search'}
      <ListPage filter="all" search />
    {:else if store.view.kind === 'settings'}
      <Settings />
    {/if}
  </main>

  <PlayerBar />

  {#if store.playerOpen}<Player />{/if}

  {#if store.importOpen}<ImportDialog />{/if}
  <ContextMenu />
  <WordPopover />
  <ConfirmDialog />

  {#if dragging && !store.importOpen}
    <div class="dropzone" transition:fade={{ duration: dur(120) }}>
      <Icon name="file" size={40} />
      <div>拖入 .txt / .md 文件以导入</div>
    </div>
  {/if}

  {#if store.toast}
    <div class="toast popup" role="status" transition:fly={{ y: 16, duration: dur(220) }}>{store.toast}</div>
  {/if}
</div>

<style>
  .window {
    position: relative;
    display: flex;
    height: 100%;
    background: var(--bg);
    color: var(--t1);
    overflow: hidden;
  }
  .main {
    flex: 1;
    min-width: 0;
    padding: 28px 32px 120px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  /* Windows / Linux 无边框窗口：右上角窗口按钮占 32px 高，主区顶部让位 */
  .window.caption-right .main {
    padding-top: 44px;
  }
  /* 窄窗口：主区边距收窄，保证表格可用 */
  @media (max-width: 1100px) {
    .main {
      padding: 24px 20px 120px;
    }
  }
  .fatal {
    padding: 60px 0;
    color: var(--danger-text);
  }
  .dropzone {
    position: fixed;
    inset: 12px;
    border: 3px dashed var(--accent);
    border-radius: 20px;
    background: var(--accent-soft);
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 10px;
    color: var(--accent-text);
    font-size: 16px;
    font-weight: 700;
    z-index: 90;
    pointer-events: none;
  }
  .toast {
    position: fixed;
    bottom: 92px;
    left: 0;
    right: 0;
    width: max-content;
    max-width: 80vw;
    margin: 0 auto;
    padding: 10px 18px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 600;
    z-index: 80;
  }
</style>
