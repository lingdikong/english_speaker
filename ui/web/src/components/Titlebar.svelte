<script lang="ts">
  // 无边框窗口的控制按钮（CEF 桌面宿主），按系统习惯摆放：
  // macOS 左上角红绿灯；Windows / Linux 右上角 最小化 / 最大化 / 关闭（关闭悬停变红）。
  // 顶部一条由宿主 apply_drag_regions 提供拖动，按钮区域挖孔；浏览器开发宿主不渲染。
  import { invoke } from '../lib/bridge';
  import { store } from '../lib/store.svelte';

  const desktop = $derived(store.host.host === 'cef');
  const mac = $derived(store.host.os === 'macos');

  // 最大化状态只用于切换图标：窗口铺满可用屏幕即视为最大化
  let maximized = $state(false);
  const measure = () =>
    (maximized = window.outerWidth >= screen.availWidth && window.outerHeight >= screen.availHeight);
  $effect(() => {
    measure();
    window.addEventListener('resize', measure);
    return () => window.removeEventListener('resize', measure);
  });
</script>

{#if desktop && mac}
  <div class="lights" role="group" aria-label="窗口控制">
    <button class="light close" aria-label="关闭" onclick={() => invoke('window_close')}></button>
    <button class="light min" aria-label="最小化" onclick={() => invoke('window_minimize')}></button>
    <button class="light max" aria-label="最大化或还原" onclick={() => invoke('window_maximize')}></button>
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
    position: relative;
    box-shadow: inset 0 0 0 0.5px rgb(0 0 0 / 14%);
  }
  .light::after {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: 9px;
    font-weight: 700;
    line-height: 1;
    color: rgb(0 0 0 / 55%);
    opacity: 0;
    transition: opacity 0.12s;
  }
  .lights:hover .light::after {
    opacity: 1;
  }
  .light.close {
    background: #ff5f57;
  }
  .light.close::after {
    content: '×';
  }
  .light.min {
    background: #febc2e;
  }
  .light.min::after {
    content: '−';
  }
  .light.max {
    background: #28c840;
  }
  .light.max::after {
    content: '+';
  }
  .light:active {
    filter: brightness(0.85);
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
