<script lang="ts">
  // 品牌图形：圆角方块内 5 根竖条声波。
  let { size = 30 }: { size?: number } = $props();
  const k = $derived(size / 30);
  const H = [8, 15, 20, 12, 7];
</script>

<div
  class="brand"
  style:width="{size}px"
  style:height="{size}px"
  style:border-radius="{Math.round(9 * k)}px"
  style:gap="{Math.max(2, Math.round(2 * k))}px"
  aria-hidden="true"
>
  {#each H as h}<i style:width="{Math.round(3 * k)}px" style:height="{Math.round(h * k)}px"></i>{/each}
</div>

<style>
  .brand {
    flex-shrink: 0;
    background: var(--accent);
    color: var(--accent-ink);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  i {
    display: block;
    border-radius: 2px;
    background: currentColor;
  }
</style>
