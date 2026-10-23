<script lang="ts">
  // 无边框窗口的控制按钮（CEF 桌面宿主），按系统习惯摆放：
  // macOS 左上角红绿灯（原生样式：渐变球面 + SVG 符号 + 失焦变灰）；
  // Windows / Linux 右上角 最小化 / 最大化 / 关闭（关闭悬停变红）。
  // 顶部一条由宿主 apply_drag_regions 提供拖动，按钮区域挖孔；浏览器开发宿主不渲染。
  import { invoke } from '../lib/bridge';
  import { store } from '../lib/store.svelte';

  const desktop = $derived(store.host.host === 'cef');
  const mac = $derived(store.host.os === 'macos');

  // 最大化状态只用于切换图标：窗口铺满可用屏幕即视为最大化
  let maximized = $state(false);
  // 窗口失焦时红绿灯退成灰色圆点，与原生行为一致
  let focused = $state(true);
  const measure = () =>
    (maximized = window.outerWidth >= screen.availWidth && window.outerHeight >= screen.availHeight);
  $effect(() => {
    measure();
    window.addEventListener('resize', measure);
    window.addEventListener('blur', () => (focused = false));
    window.addEventListener('focus', () => (focused = true));
    return () => window.removeEventListener('resize', measure);
  });
</script>

{#if desktop && mac}
  <div class="lights" class:blurred={!focused} role="group" aria-label="窗口控制">
    <button class="light close" aria-label="关闭" onclick={() => invoke('window_close')}>
      <svg viewBox="0 0 12 12" aria-hidden="true">
        <g stroke="currentColor" stroke-width="1.1" stroke-linecap="round"><path d="M4.2 4.2l3.6 3.6M7.8 4.2l-3.6 3.6" /></g>
      </svg>
    </button>
    <button class="light min" aria-label="最小化" onclick={() => invoke('window_minimize')}>
      <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M4 6h4" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" /></svg>
    </button>
    <button class="light max" aria-label="最大化或还原" onclick={() => invoke('window_maximize')}>
      <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M4 4.2 6 6 4 7.8zM8 4.2 6 6l2 1.8z" fill="currentColor" /></svg>
    </button>
  </div>
{:else if desktop}
  <div class="caption" role="group" aria-label="窗口控制">
    <button class="cap" aria-label="最小化" title="最小化" onclick={() => invoke('window_minimize')}>
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5.5h10" /></svg>
    </button>
    <button
      class="cap"
      aria-label={maximized ? '还原' : '最大化'}
      title={maximized ? '还原' : '最大化'}
      onclick={() => invoke('window_maximize')}
    >
      {#if maximized}
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M2.5 2.5V.5h7v7h-2" /><rect x=".5" y="2.5" width="7" height="7" />
        </svg>
      {:else}
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x=".5" y=".5" width="9" height="9" /></svg>
      {/if}
    </button>
    <button class="cap close" aria-label="关闭" title="关闭" onclick={() => invoke('window_close')}>
      <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M.5.5l9 9M9.5.5l-9 9" /></svg>
    </button>
  </div>
{/if}

<style>
  /* macOS：位置与宿主 apply_drag_regions 的挖孔对应：top 8 / left 20，孔 0..80 × 0..28 */
  .lights {
    position: fixed;
    top: 8px;
    left: 20px;
    display: flex;
    gap: 8px;
    z-index: 70;
  }
  .light {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    padding: 0;
    border: none;
    position: relative;
    color: transparent; /* 符号默认隐藏，悬停整组时显现 */
    transition: filter 0.12s;
  }
  .light svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  /* 原生样式：顶部高光 + 同色系 0.5px 内描边 */
  .light.close {
    background: linear-gradient(180deg, #ff7369, #ff5449);
    box-shadow:
      inset 0 0.5px 0 rgb(255 255 255 / 45%),
      inset 0 0 0 0.5px rgb(180 30 20 / 35%);
    --sym: rgb(70 0 0 / 62%);
  }
  .light.min {
    background: linear-gradient(180deg, #ffcb4f, #fdb92a);
    box-shadow:
      inset 0 0.5px 0 rgb(255 255 255 / 45%),
      inset 0 0 0 0.5px rgb(180 110 0 / 35%);
    --sym: rgb(90 50 0 / 62%);
  }
  .light.max {
    background: linear-gradient(180deg, #3ad24f, #26bd3b);
    box-shadow:
      inset 0 0.5px 0 rgb(255 255 255 / 45%),
      inset 0 0 0 0.5px rgb(0 130 20 / 35%);
    --sym: rgb(0 60 0 / 62%);
  }
  .lights:hover .light {
    color: var(--sym);
  }
  .light:active {
    filter: brightness(0.88);
  }
  /* 窗口失焦：全部退成灰色圆点 */
  .lights.blurred .light {
    background: #d5d8dc;
    box-shadow: inset 0 0 0 0.5px rgb(0 0 0 / 10%);
    color: transparent;
  }

  /* Windows / Linux：右上角三个 46×32 按钮，与宿主挖孔（右侧 138 × 32）对应 */
  .caption {
    position: fixed;
    top: 0;
    right: 0;
    display: flex;
    z-index: 70;
  }
  .cap {
    width: 46px;
    height: 32px;
    padding: 0;
    border: none;
    border-radius: 0;
    background: transparent;
    color: var(--t2);
    display: grid;
    place-items: center;
    transition:
      background-color 0.12s,
      color 0.12s;
  }
  .cap svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
    shape-rendering: crispEdges;
  }
  .cap:hover {
    background: var(--control);
    color: var(--t1);
  }
  .cap:active {
    background: var(--line);
  }
  .cap.close:hover {
    background: #c42b1c;
    color: #fff;
  }
  .cap.close:active {
    background: #a32617;
    color: #fff;
  }
  .cap:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
</style>
