<script lang="ts">
  // 合成状态 chip（UI_SPEC §3.3，替代旧版下载图标）。
  import type { ArticleSummary } from '../lib/types';
  import { store } from '../lib/store.svelte';
  import { SAVED } from '../lib/icons';

  let { a, size = 12 }: { a: ArticleSummary; size?: number } = $props();

  const playing = $derived(store.player.article_id === a.id && store.player.status !== 'idle');
  const frac = $derived(a.total_sentences ? a.ready_sentences / a.total_sentences : 0);
</script>

{#if playing}
  <span class="chip accent" style:height="24px" style:font-size="{size}px">正在播放</span>
{:else if a.status === 'queued'}
  <span class="chip accent sm" style:font-size="{size}px">排队中</span>
{:else if a.status === 'synthesizing' || a.status === 'partial'}
  <span class="chip accent sm" style:font-size="{size}px" title="已转换 {a.ready_sentences}/{a.total_sentences} 句">
    <span class="mini-prog"><b style:width="{Math.max(2, frac * 22)}px"></b></span>{a.ready_sentences}/{a.total_sentences}
  </span>
{:else if a.status === 'failed'}
  <button class="chip danger sm" style:font-size="{size}px" onclick={(e) => (e.stopPropagation(), store.synthesize(a.id))}>失败 · 重试</button>
{:else if a.status === 'not_cached'}
  <button class="chip ghost sm" style:font-size="{size}px" title="开始转换" onclick={(e) => (e.stopPropagation(), store.synthesize(a.id))}>未转换</button>
{:else if a.pinned}
  <span class="row saved" style:font-size="{size}px">{@html SAVED(12)}已保存</span>
{/if}

<style>
  .sm {
    height: 22px;
    padding: 0 8px;
  }
  button.chip:hover {
    filter: brightness(1.12);
  }
  .saved {
    gap: 4px;
    color: var(--accent-text);
    font-weight: 700;
  }
</style>
