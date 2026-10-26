<script lang="ts">
  // 左侧栏 248（UI_SPEC §5）：品牌 · 搜索 · 导航 · 分隔线 · 设置；侧栏底色与分隔线向下渐隐。
  import Brand from './Brand.svelte';
  import Icon from './Icon.svelte';
  import type { IconName } from '../lib/icons';
  import { store, type ListFilter, type View } from '../lib/store.svelte';

  let input = $state<HTMLInputElement>();
  export function focusSearch() {
    input?.focus();
    input?.select();
  }

  const nav = $derived<[IconName, string, View, number | null][]>([
    ['home', '首页', { kind: 'home' }, null],
    ['lib', '全部文章', { kind: 'list', filter: 'all' }, store.counts.all],
    ['wave', '转换中', { kind: 'list', filter: 'converting' }, store.counts.converting || null],
    ['bookmark', '已保存', { kind: 'list', filter: 'saved' }, store.counts.saved || null],
    ['cloud', '未转换', { kind: 'list', filter: 'unconverted' }, null],
  ]);

  function same(a: View, b: View) {
    // 分类页属于“全部文章”
    if (a.kind === 'collection') return b.kind === 'list' && b.filter === 'all';
    if (a.kind !== b.kind) return false;
    return a.kind !== 'list' || a.filter === (b as { filter: ListFilter }).filter;
  }

  function onSearch() {
    if (store.search.trim()) store.view = { kind: 'search' };
    else if (store.view.kind === 'search') store.view = { kind: 'list', filter: 'all' };
  }
</script>

<aside class="side fade-panel" class:desk={store.host.host === 'cef'}>
  <i class="split" style="right:0"></i>
  <div class="row brand">
    <Brand />
    <span>English Speaker</span>
  </div>
  <label class="row search">
    <Icon name="search" size={16} stroke={2} />
    <input
      bind:this={input}
      bind:value={store.search}
      oninput={onSearch}
      onkeydown={(e) => {
        if (e.key === 'Escape') {
          store.search = '';
          onSearch();
          input?.blur();
        }
      }}
      placeholder="搜索文章与原文"
      spellcheck="false"
    />
  </label>
  <nav>
    {#each nav as [icon, label, view, count]}
      {@const on = same(store.view, view)}
      <a
        href="#/"
        class="row"
        class:on
        aria-current={on ? 'page' : undefined}
        onclick={(e) => {
          e.preventDefault();
          store.view = view;
        }}
      >
        <Icon name={icon} size={18} />
        <span class="grow">{label}</span>
        {#if count != null}<span>{count}</span>{/if}
      </a>
    {/each}
    <div class="sep"></div>
    <a
      href="#/settings"
      class="row"
      class:on={store.view.kind === 'settings'}
      aria-current={store.view.kind === 'settings' ? 'page' : undefined}
      onclick={(e) => {
        e.preventDefault();
        store.view = { kind: 'settings' };
      }}
    >
      <Icon name="gear" size={18} />
      <span class="grow">设置</span>
    </a>
  </nav>
</aside>

<style>
  .side {
    position: relative;
    width: 248px;
    flex-shrink: 0;
    padding: 20px 14px;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  /* 无边框窗口：顶部为拖动条 + 红绿灯按钮（Titlebar.svelte），品牌区下移让位 */
  .side.desk {
    padding-top: 40px;
  }
  .brand {
    gap: 10px;
    padding: 0 6px;
  }
  .brand span {
    font-family: var(--display);
    font-weight: 700;
    font-size: 17px;
    letter-spacing: -0.02em;
    white-space: nowrap;
  }
  .search {
    gap: 8px;
    height: 38px;
    padding: 0 12px;
    border-radius: 12px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line);
    color: var(--t2);
  }
  .search:focus-within {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--t1);
    font-size: 14px;
    outline: none;
  }
  .search input::placeholder {
    color: var(--t2);
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  a {
    gap: 10px;
    height: 38px;
    padding: 0 12px;
    border-radius: 10px;
    text-decoration: none;
    font-size: 14px;
    font-weight: 700;
    color: var(--t2);
  }
  a:hover:not(.on) {
    background: var(--glass-edge);
    color: var(--t1);
  }
  a.on {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  a:focus-visible {
    outline-offset: -2px;
  }
  .grow {
    flex: 1;
  }
  .sep {
    height: 1px;
    background: var(--line);
    margin: 10px 12px;
  }
</style>
