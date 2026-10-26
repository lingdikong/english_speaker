<script lang="ts">
  // 设置页“翻译”组：双语显示 · 智能翻译（模型下载 / 删除）· 术语表。
  // 极速翻译（Opus-MT）与词典随包内置；智能翻译（Qwen）按需下载，未下载时整句翻译自动退回极速模式。
  import { store, fmtBytes } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  const t = $derived(store.translator);
  const llm = $derived(t?.llm);
  const pct = $derived(llm && llm.total_bytes ? Math.min(100, (llm.downloaded_bytes / llm.total_bytes) * 100) : 0);

  let term = $state('');
  let zh = $state('');
  $effect(() => {
    store.loadGlossary();
  });

  async function add(e: SubmitEvent) {
    e.preventDefault();
    if (!term.trim() || !zh.trim()) return;
    if (await store.addGlossary(term, zh)) {
      term = '';
      zh = '';
    }
  }
</script>

{#if t}
  <div class="group">
    <div class="item">
      <span>双语显示</span>
      <button
        class="switch"
        role="switch"
        aria-checked={store.prefs.bilingual}
        aria-label="双语显示"
        disabled={!store.canTranslate}
        onclick={() => store.toggleBilingual()}
      ></button>
    </div>
    <div class="item">
      <div class="col2">
        <span>智能翻译</span>
        <span class="hint">本地大模型：结合术语表精修译文，点词“举一反三”，句子解析</span>
      </div>
      <button
        class="switch"
        role="switch"
        aria-checked={t.smart_enabled}
        aria-label="智能翻译"
        onclick={() => store.setSmart(!t.smart_enabled)}
      ></button>
    </div>
    {#if llm}
      <div class="item model">
        <div class="col2" style="flex:1;min-width:0">
          <span class="row" style="gap:8px">{llm.model_name}{#if llm.loaded}<span class="chip accent sm">运行中</span>{/if}</span>
          {#if llm.state === 'downloading'}
            <div class="bar" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
              <b style:width="{pct}%"></b>
            </div>
            <span class="hint">正在下载 {fmtBytes(llm.downloaded_bytes)} / {fmtBytes(llm.total_bytes)}（{Math.floor(pct)}%）</span>
          {:else if llm.state === 'downloaded'}
            <span class="hint">已下载 · {fmtBytes(llm.total_bytes)} · 完全离线运行</span>
          {:else if llm.state === 'failed'}
            <span class="hint err">下载失败：{llm.error}</span>
          {:else}
            <span class="hint"
              >未下载 · 约 {fmtBytes(llm.total_bytes)}{#if llm.downloaded_bytes}（已下载 {fmtBytes(llm.downloaded_bytes)}，可继续）{/if}</span
            >
          {/if}
        </div>
        {#if llm.state === 'downloading'}
          <button class="chip neutral" onclick={() => store.cancelLlmDownload()}>暂停</button>
        {:else if llm.state === 'downloaded'}
          <button class="danger-btn" onclick={() => store.deleteLlm()}>删除</button>
        {:else}
          <button class="chip accent" onclick={() => store.downloadLlm()}
            ><Icon name="down" size={14} stroke={2.2} />{llm.state === 'failed' || llm.downloaded_bytes ? '继续下载' : '下载'}</button
          >
        {/if}
      </div>
    {/if}
  </div>
  <p class="note">
    {#if t.active === 'smart'}整句翻译：智能翻译（以极速译文为参考精修）。{:else if t.active === 'fast'}整句翻译：极速翻译（Opus-MT，内置）。{/if}
    {#if t.smart_enabled && t.active !== 'smart'}智能翻译模型下载完成后自动启用。{/if}
  </p>

  <div class="label" style="margin-top:8px">术语表</div>
  <div class="group">
    {#each store.glossary as g (g.term)}
      <div class="item">
        <span class="row" style="gap:10px;min-width:0"><span class="tx term">{g.term}</span><span class="arrow">→</span>{g.translation}</span>
        <button class="icon-btn" style="width:32px;height:32px" aria-label="删除术语 {g.term}" onclick={() => store.removeGlossary(g.term)}
          ><Icon name="close" size={15} stroke={2} /></button
        >
      </div>
    {/each}
    <form class="item add" onsubmit={add}>
      <input class="inp" placeholder="英文术语，如 token" bind:value={term} spellcheck="false" aria-label="英文术语" />
      <input class="inp" placeholder="固定译法，如 令牌" bind:value={zh} aria-label="固定译法" />
      <button class="chip accent" type="submit" disabled={!term.trim() || !zh.trim()}><Icon name="plus" size={14} stroke={2.4} />添加</button>
    </form>
  </div>
  <p class="note">整句翻译遇到术语时一律使用固定译法；修改后含该术语的句子会自动重新翻译。</p>
{/if}

<style>
  .col2 {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 0;
  }
  .hint {
    font-size: 12px;
    color: var(--t2);
    font-weight: 600;
    line-height: 18px;
  }
  .hint.err {
    color: var(--danger-text);
  }
  .model {
    gap: 16px;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--wave-e);
    overflow: hidden;
    margin: 4px 0 2px;
  }
  .bar b {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.25s;
  }
  .sm {
    height: 22px;
    font-size: 11px;
    padding: 0 8px;
  }
  .term {
    font-weight: 700;
  }
  .arrow {
    color: var(--t2);
  }
  .add {
    gap: 10px;
    padding: 10px 0;
  }
  .inp {
    flex: 1;
    min-width: 0;
    height: 36px;
    padding: 0 12px;
    border: none;
    border-radius: 10px;
    background: var(--control);
    color: var(--t1);
    font: inherit;
    font-size: 14px;
    outline: none;
  }
  .inp:focus {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .chip:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .note {
    margin: 0;
    font-size: 13px;
    color: var(--t2);
    line-height: 20px;
  }
  .danger-btn {
    border: none;
    background: none;
    padding: 0;
    color: var(--danger-text);
    font-size: 15px;
    font-weight: 700;
  }
  .switch:disabled {
    opacity: 0.4;
  }
</style>
