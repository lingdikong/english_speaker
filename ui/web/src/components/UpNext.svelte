<script lang="ts">
  // 待播清单：从播放条“待播清单”按钮弹出（UI_SPEC §5）。“正在播放” + 待播列表（拖动排序、移除）；播完自动播下一篇（客户端队列）。
  import { fly } from 'svelte/transition';
  import { dur } from '../lib/motion';
  import { store } from '../lib/store.svelte';
  import Thumb from './Thumb.svelte';
  import Icon from './Icon.svelte';
  import StatusChip from './StatusChip.svelte';

  const items = $derived(store.queue.map((id) => store.article(id)).filter((a) => a != null));
  /** 待播为空时给出建议：最近添加、不在播放中的文章。 */
  const suggest = $derived(
    [...store.articles]
      .filter((a) => a.id !== store.player.article_id && !store.queue.includes(a.id))
      .sort((a, b) => b.created_at_ms - a.created_at_ms)
      .slice(0, 3),
  );

  /** 在播放页里打开：贴着右上角的待播按钮弹出，而不是播放条上方。 */
  let { inPlayer = false }: { inPlayer?: boolean } = $props();

  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);
</script>

<div class="upnext popup" class:in-player={inPlayer} role="dialog" aria-label="待播清单" transition:fly={{ y: inPlayer ? -12 : 12, duration: dur(220) }}>
  <div class="row hd">
    <div class="h2" style="font-size:18px">待播清单</div>
    {#if items.length}<button class="link" onclick={() => store.setQueue([])}>清空</button>{/if}
  </div>

  {#if store.current}
    <div class="label sec">正在播放</div>
    <div class="row item now">
      <Thumb title={store.current.title} size={38} />
      <div class="meta">
        <div class="ttl" style="color:var(--accent-text)">{store.current.title}</div>
        <div class="sub">{store.voiceText(store.current)} · {store.durationText(store.current)}</div>
      </div>
    </div>
  {/if}

  <div class="label sec">接下来{items.length ? ` · ${items.length}` : ''}</div>
  <div class="list">
    {#each items as a, i (a.id)}
      <div
        class="row item"
        class:over={dragOver === i && dragFrom !== i}
        draggable="true"
        role="listitem"
        ondragstart={(e) => {
          dragFrom = i;
          e.dataTransfer!.effectAllowed = 'move';
        }}
        ondragover={(e) => {
          e.preventDefault();
          dragOver = i;
        }}
        ondragend={() => ((dragFrom = null), (dragOver = null))}
        ondrop={(e) => {
          e.preventDefault();
          if (dragFrom != null && dragFrom !== i) store.moveInQueue(dragFrom, i);
          dragFrom = dragOver = null;
        }}
        ondblclick={() => store.play(a.id)}
      >
        <span class="grip" aria-hidden="true"><Icon name="drag" size={16} /></span>
        <Thumb title={a.title} size={38} color={i % 2 ? 'var(--violet)' : 'var(--accent)'} />
        <div class="meta">
          <div class="ttl">{a.title}</div>
          <div class="row sub" style="gap:8px">{store.durationText(a)} <StatusChip {a} size={11} /></div>
        </div>
        <button class="icon-btn x" aria-label="从待播清单移除" onclick={() => store.removeFromQueue(i)}><Icon name="close" size={16} stroke={2} /></button>
      </div>
    {:else}
      <div class="empty">待播清单是空的。在文章 ⋯ 菜单中选择“加入待播清单”。</div>
      {#each suggest as a}
        <div class="row item">
          <Thumb title={a.title} size={38} color="var(--violet)" />
          <div class="meta">
            <div class="ttl">{a.title}</div>
            <div class="sub">{store.durationText(a)}</div>
          </div>
          <button class="icon-btn x" style="opacity:1" aria-label="加入待播清单" onclick={() => store.playLater(a.id)}><Icon name="plus" size={16} stroke={2.2} /></button>
        </div>
      {/each}
    {/each}
  </div>
</div>

<style>
  .upnext {
    position: fixed;
    z-index: 45;
    bottom: 88px;
    /* 与播放条右端对齐 */
    right: calc((100% - min(1080px, 100% - 48px)) / 2);
    width: 380px;
    max-height: min(560px, calc(100vh - 120px));
    padding: 16px 14px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .upnext.in-player {
    top: 92px;
    right: 32px;
    bottom: auto;
    max-height: min(560px, calc(100vh - 140px));
  }
  .hd {
    justify-content: space-between;
    padding: 0 6px 4px;
  }
  .sec {
    padding: 6px 6px 0;
  }
  .list {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    scrollbar-width: none;
  }
  .item {
    gap: 12px;
    padding: 8px 6px;
    border-radius: 12px;
  }
  .item:hover {
    background: var(--glass-edge);
  }
  .item.now {
    background: var(--accent-soft);
  }
  .item.over {
    box-shadow: inset 0 2px 0 var(--accent);
  }
  .grip {
    color: var(--t3);
    cursor: grab;
    display: flex;
    margin-right: -4px;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .ttl {
    font-weight: 700;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    font-size: 12px;
    color: var(--t2);
    margin-top: 2px;
  }
  .x {
    width: 32px;
    height: 32px;
    color: var(--t2);
    opacity: 0;
  }
  .item:hover .x,
  .x:focus-visible {
    opacity: 1;
  }
  .empty {
    padding: 8px 6px 10px;
    font-size: 13px;
    color: var(--t2);
    line-height: 20px;
  }
  .link {
    border: none;
    background: none;
    color: var(--t2);
    font-weight: 700;
    font-size: 13px;
  }
  .link:hover {
    color: var(--danger-text);
  }
</style>
