<script lang="ts">
  // 阅读设置（设置页“阅读”组与原文面板 Aa 弹层共用）：预览 · 字号 · 行距 · 非当前句亮度 · 显示跟读标注。
  import { store } from '../lib/store.svelte';

  let { compact = false }: { compact?: boolean } = $props();

  const p = $derived(store.prefs);
  const sizeName = $derived(p.fontSize <= 21 ? '较小' : p.fontSize >= 27 ? '较大' : p.fontSize === 24 ? '标准' : '适中');
  const LEVELS: [50 | 60 | 70, string][] = [
    [50, '较暗 50%'],
    [60, '适中 60%'],
    [70, '较亮 70%'],
  ];
</script>

<div class="rs" class:compact>
  <div class="preview">
    <div class="label" style="margin-bottom:10px">预览</div>
    <div class="tx cur" style:font-size="{p.fontSize - (compact ? 1 : 0)}px" style:line-height={String(p.lineHeight)}>
      <span class="read">Doctors</span> <span class="read">say</span> <span class="weak read">that</span>
      <span class="weak read">a</span> <span class="read">short</span> <span class="now">walk</span>{#if p.showNotes}<span class="lk">‿</span>{:else}{' '}{/if}<span
        class="up">after</span
      >
      <span class="up">each</span> <span class="up">meal.</span>
    </div>
    <div class="tx sent-other oth" style:line-height={String(p.lineHeight)}>It helps your heart and clears your head.</div>
  </div>

  <div class="blk">
    <div class="row" style="justify-content:space-between">
      <div class="label">字号</div>
      <span class="hint">{sizeName} · {p.fontSize}px</span>
    </div>
    <div class="row" style="gap:14px">
      <span style="font-size:14px;font-weight:700">A</span>
      <input
        class="slider"
        type="range"
        min="20"
        max="30"
        step="1"
        value={p.fontSize}
        style:--fill="{((p.fontSize - 20) / 10) * 100}%"
        style="flex:1"
        aria-label="字号"
        oninput={(e) => store.setPref('fontSize', Number((e.target as HTMLInputElement).value))}
      />
      <span style="font-size:22px;font-weight:700">A</span>
    </div>
  </div>

  <div class="blk">
    <div class="label">行距</div>
    <div class="seg">
      <button class:on={p.lineHeight === 1.6} onclick={() => store.setPref('lineHeight', 1.6)}>舒适 1.6</button>
      <button class:on={p.lineHeight === 1.4} onclick={() => store.setPref('lineHeight', 1.4)}>紧凑 1.4</button>
    </div>
  </div>

  <div class="blk">
    <div class="label">非当前句亮度</div>
    <div class="seg">
      {#each LEVELS as [v, label]}
        <button class:on={p.otherLevel === v} onclick={() => store.setPref('otherLevel', v)}>{label}</button>
      {/each}
    </div>
  </div>

  <div class="group">
    <div class="item">
      <span>显示跟读标注</span>
      <button
        class="switch"
        role="switch"
        aria-checked={p.showNotes}
        aria-label="显示跟读标注"
        onclick={() => store.toggleNotes()}
      ></button>
    </div>
  </div>
</div>

<style>
  .rs {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .compact {
    gap: 16px;
  }
  .preview {
    padding: 16px 18px;
    border-radius: 20px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .oth {
    font-size: calc(var(--tx-oth) * 1);
    margin-top: 10px;
  }
  .blk {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .hint {
    font-size: 13px;
    color: var(--t2);
  }
</style>
