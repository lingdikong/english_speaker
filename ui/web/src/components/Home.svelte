<script lang="ts">
  // 首页（UI_SPEC §5 / §4.3）：继续收听（固定尺寸方格横向列表）+ 最近添加；空资料库时显示品牌图形 + “导入第一篇文章”。
  import { store } from '../lib/store.svelte';
  import { PLAY, PAUSE } from '../lib/icons';
  import Thumb from './Thumb.svelte';
  import Brand from './Brand.svelte';
  import Icon from './Icon.svelte';
  import ArticleTable from './ArticleTable.svelte';

  const recent = $derived([...store.articles].sort((a, b) => b.created_at_ms - a.created_at_ms).slice(0, 8));

  /** 继续收听列表：当前播放的排最前，其余按进度新到旧，最多 12 格。 */
  const conts = $derived.by(() => {
    const cur = store.current;
    const rest = store.articles
      .filter(
        (a) =>
          a.id !== cur?.id &&
          (a.progress_ms ?? 0) > 0 &&
          (!a.duration_ms || (a.progress_ms ?? 0) < a.duration_ms - 1000),
      )
      .sort((a, b) => (b.progress_ms ?? 0) - (a.progress_ms ?? 0))
      .slice(0, 12);
    return cur ? [cur, ...rest] : rest;
  });
</script>

<div class="row head">
  <h1 class="h1">首页</h1>
  <button class="play row imp" onclick={() => (store.importOpen = true)}><Icon name="plus" size={16} stroke={2.4} />导入文章</button>
</div>

{#if store.loaded && !store.articles.length}
  <div class="empty">
    <Brand size={64} />
    <div class="h2">还没有文章</div>
    <p>粘贴或导入一篇英文文章，离线转换成可以边听边读的音频。</p>
    <button class="play row imp" onclick={() => (store.importOpen = true)}><Icon name="plus" size={16} stroke={2.4} />导入第一篇文章</button>
  </div>
{:else}
  {#if conts.length}
    <section>
      <div class="row sh">
        <h2 class="h2">继续收听</h2>
      </div>
      <div
        class="grid"
        onwheel={(e) => {
          // 鼠标滚轮只有纵向：横向列表里转成横向滚动（触控板本身有横向分量，不处理）
          const g = e.currentTarget;
          if (Math.abs(e.deltaX) >= Math.abs(e.deltaY) || g.scrollWidth <= g.clientWidth) return;
          const max = g.scrollWidth - g.clientWidth;
          if ((e.deltaY < 0 && g.scrollLeft <= 0) || (e.deltaY > 0 && g.scrollLeft >= max)) return;
          e.preventDefault();
          g.scrollLeft += e.deltaY;
        }}
      >
        {#each conts as a (a.id)}
          {@const cur = a.id === store.player.article_id}
          {@const pos = cur ? store.position : (a.progress_ms ?? 0)}
          {@const dur = Math.max(1, cur ? store.duration : a.duration_ms || store.details[a.id]?.estimated_ms || 0)}
          {@const left = Math.max(0, Math.ceil((dur - pos) / 60000))}
          <button class="tile" class:on={cur} onclick={() => (cur ? store.toggle() : store.play(a.id))}>
            <Thumb title={a.title} size={48} />
            <div class="tt">{a.title}</div>
            <div class="ts">{left > 0 ? `剩余 ${left} 分钟` : '即将听完'}</div>
            <div class="bar"><b style:width="{Math.min(1, pos / dur) * 100}%"></b></div>
            <span class="pplay" aria-hidden="true">
              {#if cur && store.player.status === 'buffering'}<span class="spinner" style="border-color:color-mix(in srgb,var(--accent-ink) 30%,transparent);border-top-color:var(--accent-ink)"></span>
              {:else}{@html cur && store.active ? PAUSE(20) : PLAY(20)}{/if}
            </span>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  <section>
    <div class="row sh">
      <h2 class="h2">最近添加</h2>
      <button class="link" onclick={() => (store.view = { kind: 'list', filter: 'all' })}>查看全部</button>
    </div>
    <ArticleTable items={recent} />
  </section>
{/if}

<style>
  .head {
    justify-content: space-between;
  }
  .imp {
    height: 40px;
    padding: 0 18px;
    gap: 8px;
    font-weight: 700;
    font-size: 14px;
  }
  /* 继续收听：固定尺寸方格，横向从左到右排列，放不下时横向滚动 */
  .grid {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 172px;
    gap: 14px;
    overflow-x: auto;
    padding-bottom: 4px;
    scrollbar-width: none;
  }
  .grid::-webkit-scrollbar {
    display: none;
  }
  .tile {
    position: relative;
    aspect-ratio: 1;
    border: none; /* 按钮默认边框在 Chromium 下是一圈浅色描边 */
    border-radius: 20px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line);
    padding: 16px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    cursor: pointer;
    overflow: hidden;
    text-align: center;
  }
  .tile:hover {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .tile.on {
    box-shadow: inset 0 0 0 1.5px var(--accent);
    background: var(--accent-soft);
  }
  /* 方格固定 172：48 封面 + 两行标题 36 + 剩余时间 + 进度条 + 间距 ≈ 160，不允许被压缩截断 */
  .tt {
    flex-shrink: 0;
    max-height: 36px;
    font-weight: 700;
    font-size: 14px;
    line-height: 18px;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }
  .ts {
    font-size: 12px;
    color: var(--t2);
  }
  .bar {
    width: 100%;
    height: 4px;
    border-radius: 2px;
    background: var(--wave-e);
  }
  .bar b {
    display: block;
    height: 4px;
    border-radius: 2px;
    background: var(--accent);
  }
  /* 悬停时居中的播放/暂停浮层 */
  .pplay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 35%, transparent);
    color: var(--accent-text);
    opacity: 0;
    transition: opacity 0.15s;
  }
  .pplay :global(svg) {
    padding: 12px;
    box-sizing: content-box;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-ink);
  }
  .tile:hover .pplay {
    opacity: 1;
  }
  .sh {
    justify-content: space-between;
    padding: 6px 0 12px;
  }
  .link {
    border: none;
    background: none;
    color: var(--t2);
    font-weight: 700;
    font-size: 13px;
  }
  .link:hover {
    color: var(--t1);
  }
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    text-align: center;
    padding: 80px 0;
  }
  .empty p {
    margin: 0 0 8px;
    color: var(--t2);
    font-size: 14px;
  }
</style>
