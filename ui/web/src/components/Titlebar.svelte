<script lang="ts">
  // 无边框窗口的顶栏（CEF 桌面宿主）：整条是宿主设置的拖动区（set_draggable_regions），
  // 右侧是窗口控制按钮（落在宿主挖出的非拖动孔内，可正常点击）。浏览器开发宿主不渲染。
  import { invoke } from '../lib/bridge';
  import { store } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  const desktop = $derived(store.host.host === 'cef');
</script>

{#if desktop}
  <div class="titlebar">
    <div class="controls">
      <button class="wbtn" aria-label="最小化" onclick={() => invoke('window_minimize')}
        ><Icon name="min" size={16} stroke={1.8} /></button
      >
      <button class="wbtn" aria-label="最大化或还原" onclick={() => invoke('window_maximize')}
        ><Icon name="max" size={13} stroke={1.8} /></button
      >
      <button class="wbtn close" aria-label="关闭" onclick={() => invoke('window_close')}
        ><Icon name="close" size={15} stroke={1.8} /></button
      >
    </div>
  </div>
{/if}

<style>
  /* 高度与宿主 apply_drag_regions 的 TITLEBAR 常量一致（40） */
  .titlebar {
    height: 40px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 8px;
    user-select: none;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .wbtn {
    width: 40px;
    height: 30px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    color: var(--t2);
  }
  .wbtn:hover {
    background: var(--glass-edge);
    color: var(--t1);
  }
  .wbtn.close:hover {
    background: #e5484d;
    color: #fff;
  }
</style>
