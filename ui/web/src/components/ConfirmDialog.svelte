<script lang="ts">
  // 二次确认（清除缓存、删除文章 / 分类）；带输入框时用于新建 / 重命名分类。
  import { fade, scale } from 'svelte/transition';
  import { dur } from '../lib/motion';
  import { store } from '../lib/store.svelte';

  const c = $derived(store.confirm);
  let okBtn = $state<HTMLButtonElement>();
  let field = $state<HTMLInputElement>();
  let value = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  $effect(() => {
    if (!c) return;
    value = c.input?.value ?? '';
    error = null;
    busy = false;
    queueMicrotask(() => (c.input ? (field?.focus(), field?.select()) : okBtn?.focus()));
  });

  async function ok() {
    // 先取出来：关闭对话框会把 store.confirm 置空，派生的 c 随之变成 null
    const cur = c;
    if (!cur || busy) return;
    if (cur.input) {
      error = cur.input.validate?.(value) ?? null;
      if (error) return;
    }
    // 带输入框时等操作完成再关闭，失败就把原因留在框下面；普通确认立即关闭
    if (!cur.input) {
      store.confirm = null;
      cur.action(value);
      return;
    }
    busy = true;
    try {
      await cur.action(value.trim());
      store.confirm = null;
    } catch (e) {
      error = String((e as Error).message ?? e);
    } finally {
      busy = false;
    }
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
      {#if c.body}<p>{c.body}</p>{/if}
      {#if c.input}
        <input
          class="field"
          class:bad={error}
          bind:this={field}
          bind:value
          maxlength="40"
          spellcheck="false"
          placeholder={c.input.placeholder}
          aria-label={c.title}
          aria-invalid={!!error}
          oninput={() => (error = null)}
          onkeydown={(e) => e.key === 'Enter' && ok()}
        />
        <div class="err" role="alert">{error ?? ''}</div>
      {/if}
      <div class="row btns">
        <button class="chip neutral b" onclick={() => (store.confirm = null)}>取消</button>
        <button bind:this={okBtn} class="chip b {c.danger ? 'dangerbtn' : 'accent'}" disabled={busy} onclick={ok}>{c.ok}</button>
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
  .field {
    width: 100%;
    margin-top: 16px;
  }
  .box p + .field {
    margin-top: 0;
  }
  .field.bad {
    box-shadow: inset 0 0 0 1.5px var(--danger-text);
  }
  .err {
    min-height: 20px;
    padding: 6px 4px 4px;
    font-size: 13px;
    color: var(--danger-text);
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
  .b:disabled {
    opacity: 0.6;
  }
  .dangerbtn {
    background: var(--danger-text);
    color: var(--surface);
  }
</style>
