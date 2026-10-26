<script lang="ts">
  // 分类封面（同歌单）：4 篇及以上拼成 2×2，1–3 篇用第一篇的封面，空分类用分类名生成的 violet 声波。
  import type { Collection } from '../lib/types';
  import { store } from '../lib/store.svelte';
  import Thumb from './Thumb.svelte';

  let { c, size }: { c: Collection; size: number } = $props();

  const items = $derived(store.inCollection(c.id));
  const gap = $derived(Math.max(2, Math.round(size / 40)));
  const half = $derived(Math.floor((size - gap) / 2));
</script>

{#if items.length >= 4}
  <div
    class="mosaic"
    style:width="{size}px"
    style:height="{size}px"
    style:gap="{gap}px"
    style:border-radius="{Math.round(size / 4.5)}px"
    aria-hidden="true"
  >
    {#each items.slice(0, 4) as a, i (a.id)}
      <Thumb title={a.title} size={half} color={i === 1 || i === 2 ? 'var(--violet)' : 'var(--accent)'} />
    {/each}
  </div>
{:else if items.length}
  <Thumb title={items[0].title} {size} />
{:else}
  <Thumb title={c.name} {size} color="var(--violet)" />
{/if}

<style>
  .mosaic {
    display: grid;
    grid-template-columns: 1fr 1fr;
    overflow: hidden;
    flex-shrink: 0;
  }
  /* 拼图里的小封面去掉各自的圆角，由外框统一裁成圆角方块 */
  .mosaic :global(.cover) {
    border-radius: 4px !important;
  }
</style>
