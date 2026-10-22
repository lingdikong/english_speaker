<script lang="ts">
  // 首页（UI_SPEC §5 / §4.3）：继续收听 + 最近添加；空资料库时显示品牌图形 + “导入第一篇文章”。
  import { store, locate } from '../lib/store.svelte';
  import { PLAY, PAUSE } from '../lib/icons';
  import Thumb from './Thumb.svelte';
  import Brand from './Brand.svelte';
  import Icon from './Icon.svelte';
  import ArticleTable from './ArticleTable.svelte';

  const recent = $derived([...store.articles].sort((a, b) => b.created_at_ms - a.created_at_ms).slice(0, 8));

  /** 继续收听：正在播放的文章，否则最近一篇有进度且未听完的文章。 */
  const cont = $derived.by(() => {
    if (store.current) return store.current;
    return (
      [...store.articles]
        .filter((a) => (a.progress_ms ?? 0) > 0 && (!a.duration_ms || (a.progress_ms ?? 0) < a.duration_ms - 1000))
        .sort((a, b) => (b.progress_ms ?? 0) - (a.progress_ms ?? 0))[0] ?? null
    );
  });
  const isCur = $derived(cont != null && cont.id === store.player.article_id);

  $effect(() => {
    if (cont && !store.timelines[cont.id]) store.loadTimeline(cont.id);
  });

  const info = $derived.by(() => {
    if (!cont) return null;
    const pos = isCur ? store.position : (cont.progress_ms ?? 0);
    const dur = Math.max(1, isCur ? store.duration : cont.duration_ms || (store.details[cont.id]?.estimated_ms ?? 0));
    const t = store.timelines[cont.id] ?? null;
    const s = isCur ? store.located.sentence : locate(t, pos, null).sentence;
    const total = t?.sentences.length ?? cont.total_sentences;
    const left = Math.max(0, Math.ceil((dur - pos) / 60000));
    return { frac: Math.min(1, pos / dur), text: `第 ${(s ?? 0) + 1}/${total} 句 · 剩余 ${left} 分钟` };
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
  {#if cont && info}
    <section class="panel cont">
      <div class="label">继续收听</div>
      <div class="row" style="gap:14px">
        <Thumb title={cont.title} size={72} />
        <div class="meta">
          <div class="ct">{cont.title}</div>
          <div class="cs">{info.text}</div>
        </div>
        <button
          class="play"
          style="width:48px;height:48px"
          aria-label={isCur && store.active ? '暂停' : '继续播放'}
          onclick={() => (isCur ? store.toggle() : store.play(cont.id))}
        >
          {#if isCur && store.player.status === 'buffering'}<span class="spinner" style="border-color:color-mix(in srgb,var(--accent-ink) 30%,transparent);border-top-color:var(--accent-ink)"></span>
          {:else}{@html isCur && store.active ? PAUSE(20) : PLAY(20)}{/if}
        </button>
      </div>
      <div class="bar"><b style:width="{info.frac * 100}%"></b></div>
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
  .cont {
    padding: 16px;
    border-radius: 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 720px;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .ct {
    font-family: var(--display);
    font-weight: 700;
    font-size: 18px;
    line-height: 22px;
    letter-spacing: -0.02em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cs {
    font-size: 13px;
    color: var(--t2);
    margin-top: 4px;
  }
  .bar {
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
