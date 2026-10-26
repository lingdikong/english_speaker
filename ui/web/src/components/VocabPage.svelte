<script lang="ts">
  // 生词本：点词面板里收藏的词，按收藏时间倒序。每个词带音标、简明释义与出处句子；
  // 点出处回到原文那一句播放（文章已删除时只保留例句）。
  import { store } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  let q = $state('');
  const items = $derived.by(() => {
    const k = q.trim().toLowerCase();
    if (!k) return store.vocab;
    return store.vocab.filter((v) => v.word.includes(k) || v.meaning.includes(k) || v.sentence.toLowerCase().includes(k));
  });

  function fmtDate(ms: number) {
    const d = new Date(ms);
    return `${d.getMonth() + 1}月${d.getDate()}日`;
  }

  $effect(() => {
    store.loadVocab();
  });
</script>

<div class="row head">
  <h1 class="h1">生词本</h1>
  {#if store.vocab.length}
    <label class="row search">
      <Icon name="search" size={16} stroke={2} />
      <input bind:value={q} placeholder="搜索单词、释义或例句" spellcheck="false" />
    </label>
  {/if}
</div>

{#if items.length}
  <div class="list">
    {#each items as v (v.word)}
      <div class="card">
        <div class="row top">
          <div class="row" style="gap:12px;align-items:baseline;min-width:0">
            <span class="word">{v.word}</span>
            {#if v.phonetic}<span class="ipa">/{v.phonetic}/</span>{/if}
          </div>
          <div class="row" style="gap:6px">
            <span class="date">{fmtDate(v.added_at_ms)}</span>
            <button class="icon-btn" style="width:32px;height:32px" aria-label="移出生词本" onclick={() => store.removeVocab(v.word)}
              ><Icon name="trash" size={16} /></button
            >
          </div>
        </div>
        {#if v.meaning}<div class="meaning">{v.meaning}</div>{/if}
        <div class="sent tx">{v.sentence}</div>
        {#if v.article_id && v.sentence_index != null}
          <button class="src" onclick={() => store.play(v.article_id!, v.sentence_index!)}
            ><Icon name="vol" size={14} stroke={2} />{v.article_title ?? '原文'} · 第 {v.sentence_index + 1} 句</button
          >
        {/if}
      </div>
    {/each}
  </div>
{:else if store.loaded}
  <div class="empty">
    {q ? '没有找到匹配的词。' : '还没有生词。播放时点原文里的词，在弹出的卡片上点 ☆ 收藏。'}
  </div>
{/if}

<style>
  .head {
    justify-content: space-between;
    gap: 16px;
  }
  .search {
    gap: 8px;
    height: 38px;
    width: 280px;
    padding: 0 12px;
    border-radius: 12px;
    background: var(--control);
    color: var(--t2);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: none;
    background: none;
    outline: none;
    color: var(--t1);
    font: inherit;
    font-size: 14px;
  }
  .list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 14px;
    max-width: 1200px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px 18px;
    border-radius: 20px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .top {
    justify-content: space-between;
    gap: 10px;
  }
  .word {
    font-family: var(--display);
    font-weight: 700;
    font-size: 22px;
    letter-spacing: -0.02em;
  }
  .ipa {
    font-family: var(--ipa);
    color: var(--violet-text);
    font-size: 15px;
  }
  .date {
    font-size: 12px;
    color: var(--t2);
    font-weight: 600;
  }
  .meaning {
    font-size: 14px;
    line-height: 22px;
  }
  .sent {
    font-size: 14px;
    line-height: 22px;
    color: var(--t2);
  }
  .src {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: none;
    background: none;
    padding: 0;
    color: var(--accent-text);
    font-size: 13px;
    font-weight: 700;
    cursor: pointer;
  }
  .src:hover {
    text-decoration: underline;
  }
  .empty {
    padding: 60px 0;
    color: var(--t2);
    font-size: 14px;
  }
</style>
