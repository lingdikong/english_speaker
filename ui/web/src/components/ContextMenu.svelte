<script lang="ts">
  // 文章更多操作菜单（⋯ / 右键）：保存 / 取消保存、换音色（子菜单）、删除缓存、删除文章等。
  import { store, type MenuItem } from '../lib/store.svelte';

  let el = $state<HTMLDivElement>();
  let pos = $state({ x: 0, y: 0 });
  let sub = $state<{ items: MenuItem[]; x: number; y: number } | null>(null);

  $effect(() => {
    const m = store.menu;
    sub = null;
    if (!m || !el) return;
    const r = el.getBoundingClientRect();
    pos = {
      x: Math.max(8, Math.min(m.x, innerWidth - r.width - 8)),
      y: m.y + r.height > innerHeight - 8 ? Math.max(8, m.y - r.height) : m.y,
    };
  });

  function run(it: MenuItem) {
    if (it.disabled || it.submenu) return;
    store.menu = null;
    it.action?.();
  }

  function openSub(e: MouseEvent, it: MenuItem) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    if (!it.submenu) {
      sub = null;
      return;
    }
    const w = 230;
    const x = r.right + w + 4 > innerWidth ? r.left - w - 4 : r.right + 4;
    const h = it.submenu.length * 34 + 12;
    sub = { items: it.submenu, x, y: Math.max(8, Math.min(r.top - 6, innerHeight - h - 8)) };
  }
</script>

<svelte:window
  onmousedown={(e) => {
    if (store.menu && !(e.target as HTMLElement).closest('.menu')) store.menu = null;
  }}
  onblur={() => (store.menu = null)}
/>

{#if store.menu}
  <div class="menu popup" bind:this={el} style:left="{pos.x}px" style:top="{pos.y}px" role="menu">
    {#each store.menu.items as it}
      {#if it.separator}
        <div class="sep"></div>
      {:else}
        <button
          class="mi"
          class:danger={it.danger}
          class:open={sub && it.submenu === sub.items}
          disabled={it.disabled}
          role="menuitem"
          onmouseenter={(e) => openSub(e, it)}
          onclick={(e) => (it.submenu ? openSub(e, it) : run(it))}
        >
          <span class="lbl">{it.label}</span>
          {#if it.submenu}<span class="chev">›</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
  {#if sub}
    <div class="menu popup subm" style:left="{sub.x}px" style:top="{sub.y}px" role="menu">
      {#each sub.items as it}
        <button class="mi" class:checked={it.checked} disabled={it.disabled} role="menuitemradio" aria-checked={!!it.checked} onclick={() => run(it)}>
          <span class="lbl">{it.label}</span>
          {#if it.checked}<span class="chev">✓</span>{/if}
        </button>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 100;
    min-width: 210px;
    padding: 6px;
    border-radius: 16px;
    font-size: 14px;
  }
  .subm {
    width: 230px;
    max-height: 60vh;
    overflow-y: auto;
  }
  .mi {
    display: flex;
    align-items: center;
    width: 100%;
    height: 34px;
    padding: 0 12px;
    border: none;
    border-radius: 10px;
    background: transparent;
    color: var(--t1);
    font-weight: 600;
    font-size: 14px;
    text-align: left;
    white-space: nowrap;
  }
  .mi:hover:not(:disabled),
  .mi.open {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .mi:disabled {
    color: var(--t3);
    cursor: default;
  }
  .mi.danger {
    color: var(--danger-text);
  }
  .mi.danger:hover {
    background: var(--danger-soft);
    color: var(--danger-text);
  }
  .mi.checked {
    color: var(--accent-text);
  }
  .lbl {
    flex: 1;
    padding-right: 16px;
  }
  .chev {
    color: inherit;
    font-weight: 700;
  }
  .sep {
    height: 1px;
    background: var(--line);
    margin: 5px 10px;
  }
</style>
