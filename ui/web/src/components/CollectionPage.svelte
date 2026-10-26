<script lang="ts">
  // 分类详情（同歌单页）：返回“全部文章” · 大封面 + 名称 + 篇数 / 总时长 · 播放全部 · ⋯（重命名 / 删除）· 文章表格。
  import type { Collection } from '../lib/types';
  import { store, fmtTime } from '../lib/store.svelte';
  import { MORE, PAUSE, PLAY } from '../lib/icons';
  import CollectionCover from './CollectionCover.svelte';
  import ArticleTable from './ArticleTable.svelte';
  import Icon from './Icon.svelte';

  let { c }: { c: Collection } = $props();

  const items = $derived(store.inCollection(c.id));
  const total = $derived(items.reduce((s, a) => s + Math.max(a.duration_ms, store.details[a.id]?.estimated_ms ?? 0), 0));
  const playing = $derived(items.some((a) => a.id === store.player.article_id) && store.active);
</script>

<button class="back row" onclick={() => (store.view = { kind: 'list', filter: 'all' })}>
  <span class="chev" aria-hidden="true">‹</span>全部文章
</button>

<header class="row hero">
  <CollectionCover {c} size={184} />
  <div class="meta">
    <div class="label">分类</div>
    <h1 class="h1" title={c.name}>{c.name}</h1>
    <div class="sub">{items.length} 篇{total ? ` · 约 ${fmtTime(total)}` : ''}</div>
    <div class="row acts">
      <button class="play row pa" disabled={!items.length} onclick={() => (playing ? store.toggle() : store.playCollection(c.id))}>
        {@html playing ? PAUSE(16) : PLAY(16)}{playing ? '暂停' : '播放全部'}
      </button>
      <button
        class="icon-btn fill"
        aria-label="更多操作"
        onclick={(e) => {
          const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
          store.menu = { x: r.left, y: r.bottom + 6, items: store.collectionMenu(c) };
        }}>{@html MORE}</button
      >
    </div>
  </div>
</header>

{#if items.length}
  <ArticleTable {items} collectionId={c.id} />
{:else}
  <div class="empty">
    <Icon name="lib" size={28} />
    <p>这个分类还是空的。<br />在文章的 ⋯ 菜单里选“添加到分类”，或在“全部文章”里把文章拖到分类上。</p>
  </div>
{/if}

<style>
  .back {
    align-self: flex-start;
    gap: 4px;
    padding: 4px 10px 4px 6px;
    margin: -6px 0 -4px -6px;
    border: none;
    border-radius: 999px;
    background: none;
    color: var(--t2);
    font-weight: 700;
    font-size: 13px;
  }
  .back:hover {
    color: var(--t1);
    background: var(--glass-edge);
  }
  .chev {
    font-size: 20px;
    line-height: 16px;
  }
  .hero {
    gap: 28px;
    align-items: flex-end;
    padding-bottom: 8px;
  }
  .meta {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .h1 {
    font-size: 36px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 14px;
    font-weight: 600;
    color: var(--t2);
  }
  .acts {
    gap: 10px;
    margin-top: 12px;
  }
  .pa {
    height: 40px;
    padding: 0 20px 0 16px;
    gap: 8px;
    font-weight: 700;
    font-size: 14px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 56px 12px;
    color: var(--t2);
    text-align: center;
  }
  .empty p {
    margin: 0;
    font-size: 14px;
    line-height: 22px;
  }
</style>
