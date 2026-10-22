<script lang="ts">
  // 二次确认（清除缓存、删除文章）。
  import { fade, scale } from 'svelte/transition';
  import { dur } from '../lib/motion';
  import { store } from '../lib/store.svelte';

  const c = $derived(store.confirm);
  let okBtn = $state<HTMLButtonElement>();
  $effect(() => {
    if (c) okBtn?.focus();
  });

  function ok() {
    const a = c?.action;
    store.confirm = null;
    a?.();
  }
</script>

{#if c}
  <div class="scrim" transition:fade={{ duration: dur(150) }} onclick={() => (store.confirm = null)} role="presentation"></div>
  <div class="wrap" role="presentation">
    <div
      class="box"
      role="alertdialog"
      aria-modal="true"
      aria-label={c.title}
      tabindex="-1"
      transition:scale={{ start: 0.96, duration: dur(220), opacity: 0 }}
      onkeydown={(e) => {
        if (e.key === 'Escape') store.confirm = null;
        e.stopPropagation();
      }}
    >
      <div class="h2" style="font-size:19px">{c.title}</div>
      <p>{c.body}</p>
      <div class="row btns">
        <button class="chip neutral b" onclick={() => (store.confirm = null)}>取消</button>
        <button bind:this={okBtn} class="chip b {c.danger ? 'dangerbtn' : 'accent'}" onclick={ok}>{c.ok}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.45);
    z-index: 110;
  }
  .wrap {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    z-index: 111;
    pointer-events: none;
  }
  .box {
    pointer-events: auto;
    width: 400px;
    padding: 22px;
    border-radius: 22px;
    background: var(--surface);
    box-shadow:
      inset 0 0 0 1px var(--line),
      0 30px 80px rgba(0, 0, 0, 0.35);
    outline: none;
  }
  p {
    margin: 10px 0 20px;
    font-size: 14px;
    line-height: 22px;
    color: var(--t2);
  }
  .btns {
    gap: 10px;
    justify-content: flex-end;
  }
  .b {
    height: 40px;
    padding: 0 18px;
    font-size: 14px;
  }
  .dangerbtn {
    background: var(--danger-text);
    color: var(--surface);
  }
</style>
