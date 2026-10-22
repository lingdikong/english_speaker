<script lang="ts">
  // 声波进度条（UI_SPEC §3.2）：种子 = 文章 id；已播 accent / 已缓冲 wave-b / 未合成 wave-e；点按 / 拖动 → seek。
  import { store } from '../lib/store.svelte';
  import { waveOf } from '../lib/wave';

  let { n = 120, height = 10, min = 2, gap = 2 }: { n?: number; height?: number; min?: number; gap?: number } = $props();

  const id = $derived(store.player.article_id ?? '');
  const hs = $derived(waveOf(id, n, min, height));
  const dur = $derived(Math.max(1, store.duration));
  const played = $derived(id ? Math.floor((store.position / dur) * n) : 0);
  const buffered = $derived(id ? Math.floor((store.buffered / dur) * n) : 0);

  let el = $state<HTMLDivElement>();

  function msAt(x: number): number {
    const r = el!.getBoundingClientRect();
    const f = Math.min(1, Math.max(0, (x - r.left) / r.width));
    return Math.min(f * dur, store.buffered);
  }

  function down(e: PointerEvent) {
    if (!id || e.button !== 0) return;
    el!.setPointerCapture(e.pointerId);
    store.scrubbing = msAt(e.clientX);
  }
  function move(e: PointerEvent) {
    if (store.scrubbing != null) store.scrubbing = msAt(e.clientX);
  }
  function up(e: PointerEvent) {
    if (store.scrubbing == null) return;
    const ms = msAt(e.clientX);
    store.scrubbing = null;
    store.seek(ms);
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      e.preventDefault();
      e.stopPropagation();
      store.skip(e.key === 'ArrowLeft' ? -5000 : 5000);
    }
  }
</script>

<div
  class="wave bottom wp"
  bind:this={el}
  style:height="{height}px"
  style:gap="{gap}px"
  role="slider"
  tabindex="0"
  aria-label="播放进度"
  aria-valuemin={0}
  aria-valuemax={Math.round(dur / 1000)}
  aria-valuenow={Math.round(store.position / 1000)}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={() => (store.scrubbing = null)}
  onkeydown={key}
>
  {#each hs as h, i}
    <i style:height="{h}px" style:background={i < played ? 'var(--accent)' : i < buffered ? 'var(--wave-b)' : 'var(--wave-e)'}></i>
  {/each}
</div>

<style>
  .wp {
    flex: 1;
    min-width: 0;
    cursor: pointer;
    touch-action: none;
    /* 加大点按区域，不改变视觉高度 */
    padding: 6px 0;
    margin: -6px 0;
    box-sizing: content-box;
  }
</style>
