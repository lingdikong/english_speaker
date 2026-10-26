<script lang="ts">
  // 文章表格（UI_SPEC §5）：封面 48 / 标题 / 音色 / 时长 / 状态（+ 悬停时的 ⋯）。点按行播放，右键 / ⋯ 打开更多操作；
  // 行可拖到分类卡片上移入分类。
  import type { ArticleSummary } from '../lib/types';
  import { store, ARTICLE_MIME } from '../lib/store.svelte';
  import { MORE } from '../lib/icons';
  import Thumb from './Thumb.svelte';
  import Icon from './Icon.svelte';
  import StatusChip from './StatusChip.svelte';

  /** showCollections：标题下注明所在分类（全部文章页）；collectionId：在该分类页里，菜单多一项“从此分类移除”。 */
  let {
    items,
    query = '',
    showCollections = false,
    collectionId,
  }: { items: ArticleSummary[]; query?: string; showCollections?: boolean; collectionId?: string } = $props();

  /** 命中片段：正文中第一个匹配处前后各取一段。 */
  function snippet(a: ArticleSummary, q: string): [string, string, string] | null {
    const body = store.details[a.id]?.body ?? '';
    const i = body.toLowerCase().indexOf(q);
    if (i < 0) return null;
    const s = Math.max(0, i - 40);
    const e = Math.min(body.length, i + q.length + 70);
    return [(s > 0 ? '…' : '') + body.slice(s, i), body.slice(i, i + q.length), body.slice(i + q.length, e) + (e < body.length ? '…' : '')];
  }

  function parts(text: string, q: string): [string, string, string] | null {
    const i = text.toLowerCase().indexOf(q);
    return i < 0 ? null : [text.slice(0, i), text.slice(i, i + q.length), text.slice(i + q.length)];
  }

  const q = $derived(query.trim().toLowerCase());
</script>

<div class="table" role="table" aria-label="文章">
  <div class="tr head" role="row">
    <span></span><span role="columnheader">标题</span><span role="columnheader" class="c-voice">音色</span><span
      role="columnheader"
      class="c-dur">时长</span
    ><span role="columnheader">状态</span>
  </div>
  {#each items as a, i (a.id)}
    {@const now = store.player.article_id === a.id && store.player.status !== 'idle'}
    {@const tp = q ? parts(a.title, q) : null}
    {@const sn = q && !tp ? snippet(a, q) : null}
    {@const cols = showCollections ? store.collectionsOf(a.id) : []}
    <div
      class="tr"
      class:now
      role="row"
      tabindex="0"
      draggable="true"
      ondragstart={(e) => {
        e.dataTransfer!.setData(ARTICLE_MIME, a.id);
        e.dataTransfer!.effectAllowed = 'copy';
      }}
      onclick={() => store.play(a.id)}
      onkeydown={(e) => e.key === 'Enter' && store.play(a.id)}
      oncontextmenu={(e) => store.openMenu(e, store.articleMenu(a, collectionId))}
    >
      <Thumb title={a.title} size={48} color={now || i % 2 === 0 ? 'var(--accent)' : 'var(--violet)'} />
      <span class="title-cell" role="cell">
        <span class="title">
          {#if tp}{tp[0]}<mark>{tp[1]}</mark>{tp[2]}{:else}{a.title}{/if}
        </span>
        {#if sn}<span class="snip">{sn[0]}<mark>{sn[1]}</mark>{sn[2]}</span>{/if}
        {#if cols.length}<span class="col"><Icon name="lib" size={12} stroke={2} />{cols.map((c) => c.name).join(' · ')}</span>{/if}
      </span>
      <span class="sub c-voice" role="cell">{store.voiceText(a)}</span>
      <span class="sub c-dur" role="cell">{store.durationText(a)}</span>
      <span role="cell"><StatusChip {a} /></span>
      <button
        class="icon-btn more"
        aria-label="更多操作"
        onclick={(e) => {
          e.stopPropagation();
          const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
          store.menu = { x: r.right - 210, y: r.bottom + 4, items: store.articleMenu(a, collectionId) };
        }}>{@html MORE}</button
      >
    </div>
  {/each}
</div>

<style>
  .table {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .tr {
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr) 150px 80px 150px;
    position: relative;
    gap: 16px;
    align-items: center;
    padding: 10px 12px;
    border-radius: 14px;
    cursor: default;
  }
  .tr:not(.head):hover {
    background: var(--glass-edge);
  }
  .tr.now,
  .tr.now:hover {
    background: var(--accent-soft);
  }
  .tr:focus-visible {
    outline-offset: -2px;
  }
  .head {
    padding: 0 12px 10px;
    border-radius: 0;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: var(--t2);
    box-shadow: inset 0 -1px 0 var(--line);
  }
  .title-cell {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .title {
    font-weight: 700;
    font-size: 15px;
    line-height: 20px;
    overflow: hidden;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }
  .now .title {
    color: var(--accent-text);
  }
  .snip {
    font-size: 13px;
    color: var(--t2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .col {
    display: block;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    font-weight: 600;
    color: var(--violet-text);
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  mark {
    background: var(--accent-soft);
    color: var(--accent-text);
    border-radius: 6px;
    padding: 0 3px;
  }
  .sub {
    font-size: 14px;
    color: var(--t2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .more {
    position: absolute;
    right: 8px;
    top: calc(50% - 16px);
    width: 32px;
    height: 32px;
    color: var(--t2);
    opacity: 0;
  }
  .tr:hover .more,
  .more:focus-visible {
    opacity: 1;
  }
  @container (max-width: 620px) {
    .tr {
      grid-template-columns: 56px minmax(0, 1fr) 80px 130px;
    }
    .c-voice {
      display: none;
    }
  }
  @container (max-width: 440px) {
    .tr {
      grid-template-columns: 56px minmax(0, 1fr) 130px;
    }
    .c-dur {
      display: none;
    }
  }
</style>
