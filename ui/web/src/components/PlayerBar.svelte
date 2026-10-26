<script lang="ts">
  // 悬浮玻璃播放条（UI_SPEC §5）：药丸形，高 60，宽 min(1080, 窗口 − 48)，相对整个窗口水平居中，bottom 16，最上层。
  // 左：单句循环 · 上一句 · 播放/暂停 · 下一句 · 倍速；中：封面（点击展开播放页）· 标题 · 声波进度；右：待播 · 音量。
  import { store, fmtTime } from '../lib/store.svelte';
  import { G } from '../lib/icons';
  import Thumb from './Thumb.svelte';
  import WaveProgress from './WaveProgress.svelte';
  import UpNext from './UpNext.svelte';

  const a = $derived(store.current);
  const t = $derived(store.timeline);
  const cur = $derived(store.located.sentence);
  const sub = $derived(
    [store.voiceText(a) === '—' ? '' : store.voiceText(a), t && cur != null ? `第 ${cur + 1}/${t.sentences.length} 句` : '']
      .filter(Boolean)
      .join(' · '),
  );
  const remain = $derived(Math.max(0, store.duration - store.position));
  const rate = $derived(store.player.rate);
  const rateText = $derived(`${Math.abs(rate - Math.round(rate * 10) / 10) < 0.001 ? rate.toFixed(1) : rate.toFixed(2)}×`);

  let volOpen = $state(false);
  let volHover = 0;
  function volEnter() {
    clearTimeout(volHover);
    volOpen = true;
  }
  function volLeave() {
    clearTimeout(volHover);
    volHover = window.setTimeout(() => (volOpen = false), 350);
  }
  const vol = $derived(store.host.volume);
</script>

<svelte:window
  onmousedown={(e) => {
    const el = e.target as HTMLElement;
    if (store.queueOpen && !el.closest('.upnext, [data-queue-btn]')) store.queueOpen = false;
    if (volOpen && !el.closest('.vol-wrap')) volOpen = false;
  }}
/>

<div class="glassbar glass" role="region" aria-label="播放控制">
  <div class="row" style="gap:4px">
    <button
      class="gbtn"
      style:color={store.loop ? 'var(--accent-text)' : undefined}
      aria-label={store.loop ? '单句循环（已开启）' : '单句循环'}
      aria-pressed={store.loop}
      title="单句循环（L）"
      onclick={() => store.toggleLoop()}>{@html G.loop}</button
    >
    <button class="gbtn" aria-label="上一句" title="上一句（⌘/Ctrl + ←）" disabled={!a} onclick={() => store.prevSentence()}>{@html G.prev}</button>
    <button
      class="gbtn main"
      aria-label={store.player.status === 'buffering' ? '缓冲中' : store.active ? '暂停' : '播放'}
      title="播放 / 暂停（空格）"
      onclick={() => store.toggle()}
    >
      {#if store.player.status === 'buffering'}<span class="spinner" aria-hidden="true"></span>
      {:else}{@html store.active ? G.pause : G.play}{/if}
    </button>
    <button class="gbtn" aria-label="下一句" title="下一句（⌘/Ctrl + →）" disabled={!a} onclick={() => store.nextSentence()}>{@html G.next}</button>
    <button class="gbtn rate" aria-label="倍速 {rateText}" title="倍速" onclick={() => store.cycleRate()}>{rateText}</button>
  </div>

  <div class="row mid">
    {#if a}
      <button
        class="art"
        data-player-origin
        aria-label="打开播放页"
        title="打开播放页"
        aria-expanded={store.playerOpen}
        onclick={() => store.openPlayer()}
      >
        <Thumb title={a.title} size={38} />
        <span class="expand" aria-hidden="true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="m6 15 6-6 6 6" /></svg>
        </span>
      </button>
      <div class="info">
        <div class="row" style="gap:10px;min-width:0">
          <span class="ttl">{a.title}</span>
          <span class="sub">{sub}</span>
        </div>
        <div class="row times">
          <span>{fmtTime(store.position)}</span>
          <WaveProgress />
          <span>{store.player.duration_final ? '' : '约 '}-{fmtTime(remain)}</span>
        </div>
      </div>
    {:else}
      <div class="cover idle" aria-hidden="true"></div>
      <div class="info">
        <span class="ttl" style="color:var(--t2)">未在播放</span>
        <div class="row times"><span>0:00</span><WaveProgress /><span>-0:00</span></div>
      </div>
    {/if}
  </div>

  <div class="row" style="gap:6px">
    <button
      class="gbtn"
      data-queue-btn
      aria-label="待播清单"
      aria-expanded={store.queueOpen}
      title="待播清单"
      onclick={() => (store.queueOpen = !store.queueOpen)}>{@html G.queue}</button
    >
    <div class="vol-wrap" role="group" aria-label="音量" onmouseenter={volEnter} onmouseleave={volLeave}>
      <button
        class="gbtn"
        aria-label="音量 {Math.round(vol * 100)}%"
        aria-expanded={volOpen}
        onclick={() => (volOpen = !volOpen)}>{@html vol <= 0.001 ? G.mute : G.vol}</button
      >
      {#if volOpen}
        <div class="vol popup">
          <span class="vv">{Math.round(vol * 100)}</span>
          <input
            class="slider vs"
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={vol}
            style:--fill="{vol * 100}%"
            aria-label="音量"
            oninput={(e) => store.setVolume(Number((e.target as HTMLInputElement).value))}
          />
        </div>
      {/if}
    </div>
  </div>
</div>

{#if store.queueOpen && !store.playerOpen}<UpNext />{/if}

<style>
  .glassbar {
    position: fixed;
    left: 0;
    right: 0;
    margin: 0 auto;
    width: min(1080px, calc(100% - 48px));
    bottom: 16px;
    height: 60px;
    border-radius: 999px;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 24px;
    padding: 0 22px;
    z-index: 40;
  }
  .gbtn {
    width: 40px;
    height: 40px;
    border-radius: 999px;
    border: none;
    background: transparent;
    color: var(--t1);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: background-color 0.2s, color 0.2s;
  }
  .gbtn:hover:not(:disabled) {
    background: var(--glass-edge);
  }
  .gbtn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .gbtn.main {
    width: 44px;
    height: 44px;
  }
  .rate {
    width: auto;
    min-width: 48px;
    padding: 0 10px;
    font-weight: 700;
    font-size: 14px;
  }
  .mid {
    gap: 14px;
    min-width: 0;
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .ttl {
    font-weight: 700;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .sub {
    font-size: 12px;
    color: var(--t2);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .times {
    gap: 10px;
    font-size: 11px;
    font-weight: 700;
    color: var(--t2);
    font-variant-numeric: tabular-nums;
  }
  .times > span {
    white-space: nowrap;
  }
  /* 封面：播放页入口。悬停时轻微放大并浮出“展开”箭头 */
  .art {
    position: relative;
    padding: 0;
    border: none;
    background: none;
    border-radius: 8px;
    flex-shrink: 0;
    transition: transform 0.35s var(--spring);
  }
  .art:hover {
    transform: scale(1.08);
  }
  .art:active {
    transform: scale(0.96);
  }
  .expand {
    position: absolute;
    inset: 0;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 55%, transparent);
    color: var(--t1);
    opacity: 0;
    transition: opacity 0.18s;
  }
  .art:hover .expand,
  .art:focus-visible .expand {
    opacity: 1;
  }
  .idle {
    width: 38px;
    height: 38px;
    border-radius: 8px;
  }
  .vol-wrap {
    position: relative;
  }
  .vol {
    position: absolute;
    bottom: 52px;
    left: 50%;
    transform: translateX(-50%);
    width: 52px;
    height: 180px;
    border-radius: 999px;
    padding: 14px 0 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }
  .vv {
    font-size: 11px;
    font-weight: 700;
    color: var(--t2);
    font-variant-numeric: tabular-nums;
  }
  .vs {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 6px;
    flex: 1;
    background: linear-gradient(to top, var(--accent) var(--fill, 50%), var(--wave-e) var(--fill, 50%));
  }
  /* 窄窗口：先收起副标题 */
  @media (max-width: 1100px) {
    .sub {
      display: none;
    }
  }
</style>
