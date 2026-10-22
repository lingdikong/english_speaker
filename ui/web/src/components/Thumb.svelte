<script lang="ts">
  // 生成封面（UI_SPEC §3.1）：card 底 + line 内描边，圆角 = 边长/4.5，内部一排确定性竖条声波。
  import { thumbBars, waveOf } from '../lib/wave';

  let { title, size, color = 'var(--accent)' }: { title: string; size: number; color?: string } = $props();

  const n = $derived(thumbBars(size));
  const min = $derived(Math.round(size / 10));
  const max = $derived(size - Math.round(size / 2.6));
  const hs = $derived(waveOf(title, n, min, max));
</script>

<div
  class="cover"
  style:width="{size}px"
  style:height="{size}px"
  style:border-radius="{Math.round(size / 4.5)}px"
  style:padding="0 {Math.round(size / 7)}px"
  aria-hidden="true"
>
  <div class="wave" style:height="{max}px" style:width="100%">
    {#each hs as h}<i style:height="{h}px" style:background={color}></i>{/each}
  </div>
</div>
