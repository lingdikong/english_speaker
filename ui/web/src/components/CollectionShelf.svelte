<script lang="ts">
  // 全部文章页顶部的“分类”（同歌单）：新建分类 + 每个分类一张卡片（封面 · 名称 · 篇数），横向排列、放不下时横向滚动。
  // 点卡片进入分类；悬停出现“播放全部”；把文章行拖到卡片上即加入该分类（一篇可在多个分类里）。
  import { store, ARTICLE_MIME } from '../lib/store.svelte';
  import { PLAY } from '../lib/icons';
  import CollectionCover from './CollectionCover.svelte';
  import Icon from './Icon.svelte';

  let over = $state<string | null>(null);

  function dragover(e: DragEvent, id: string) {
    if (!e.dataTransfer?.types.includes(ARTICLE_MIME)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
    over = id;
  }

  function drop(e: DragEvent, id: string) {
    const article = e.dataTransfer?.getData(ARTICLE_MIME);
    over = null;
    if (!article) return;
    e.preventDefault();
    store.addToCollection(article, id);
  }

  // 鼠标滚轮只有纵向：转成横向滚动（触控板本身有横向分量，不处理）
  function wheel(e: WheelEvent) {
    const g = e.currentTarget as HTMLElement;
    if (Math.abs(e.deltaX) >= Math.abs(e.deltaY) || g.scrollWidth <= g.clientWidth) return;
    const max = g.scrollWidth - g.clientWidth;
    if ((e.deltaY < 0 && g.scrollLeft <= 0) || (e.deltaY > 0 && g.scrollLeft >= max)) return;
    e.preventDefault();
    g.scrollLeft += e.deltaY;
  }
</script>

<section>
  <div class="row sh">
    <h2 class="h2">分类</h2>
    {#if store.collections.length}<span class="hint">把文章拖到分类上即可添加，一篇可放进多个分类</span>{/if}
  </div>
  <div class="shelf" onwheel={wheel}>
    <button class="card new" onclick={() => store.newCollection()}>
      <span class="cv"><Icon name="plus" size={30} stroke={2} /></span>
      <span class="nm">新建分类</span>
      <span class="ct">像歌单一样整理文章</span>
    </button>
    {#each store.collections as c (c.id)}
      {@const n = store.inCollection(c.id).length}
      <div
        class="card"
        class:over={over === c.id}
        role="button"
        tabindex="0"
        aria-label="{c.name}，{n} 篇"
        onclick={() => (store.view = { kind: 'collection', id: c.id })}
        onkeydown={(e) => e.key === 'Enter' && (store.view = { kind: 'collection', id: c.id })}
        oncontextmenu={(e) => store.openMenu(e, store.collectionMenu(c))}
        ondragover={(e) => dragover(e, c.id)}
        ondragleave={() => over === c.id && (over = null)}
        ondrop={(e) => drop(e, c.id)}
      >
        <span class="cv">
          <CollectionCover {c} size={156} />
          {#if n}
            <button
              class="pp"
              aria-label="播放“{c.name}”"
              title="播放全部"
              onclick={(e) => {
                e.stopPropagation();
                store.playCollection(c.id);
              }}>{@html PLAY(18)}</button
            >
          {/if}
        </span>
        <span class="nm" title={c.name}>{c.name}</span>
        <span class="ct">{n ? `${n} 篇` : '空分类'}</span>
      </div>
    {/each}
  </div>
</section>

<style>
  .sh {
    gap: 12px;
    padding: 6px 0 12px;
  }
  .hint {
    font-size: 12px;
    color: var(--t3);
  }
  .shelf {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 172px;
    gap: 14px;
    overflow-x: auto;
    padding: 2px 2px 6px;
    margin: -2px -2px 0;
    scrollbar-width: none;
  }
  .shelf::-webkit-scrollbar {
    display: none;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px;
    border: none;
    border-radius: 20px;
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition:
      background-color 0.2s,
      box-shadow 0.2s;
  }
  .card:hover {
    background: var(--glass-edge);
  }
  /* 拖着文章悬停在分类上 */
  .card.over {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .cv {
    position: relative;
    display: block;
    width: 156px;
    height: 156px;
    margin-bottom: 6px;
    border-radius: 35px;
    transition: transform 0.35s var(--spring);
  }
  .card.over .cv {
    transform: scale(1.04);
  }
  .new .cv {
    display: grid;
    place-items: center;
    color: var(--t2);
    background: var(--surface);
    border: 1.5px dashed var(--wave-b);
  }
  .new:hover .cv {
    color: var(--accent-text);
    border-color: var(--accent);
  }
  .nm {
    font-weight: 700;
    font-size: 14px;
    line-height: 19px;
    padding: 0 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ct {
    font-size: 12px;
    color: var(--t2);
    padding: 0 2px;
  }
  /* 悬停时封面右下角浮出“播放全部” */
  .pp {
    position: absolute;
    right: 10px;
    bottom: 10px;
    width: 40px;
    height: 40px;
    padding: 0;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--accent-ink);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.3);
    opacity: 0;
    transform: translateY(6px);
    transition:
      opacity 0.18s,
      transform 0.25s var(--spring);
  }
  .card:hover .pp,
  .pp:focus-visible {
    opacity: 1;
    transform: none;
  }
</style>
