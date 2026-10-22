<script lang="ts">
  // 文章列表：全部 / 转换中 / 已保存 / 未转换（侧栏导航与筛选 chip 同步），以及搜索结果（标题 + 正文，命中处高亮）。
  import { store, isConverting, type ListFilter } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import ArticleTable from './ArticleTable.svelte';

  let { filter, search = false }: { filter: ListFilter; search?: boolean } = $props();

  const TITLES: Record<ListFilter, string> = { all: '全部文章', converting: '转换中', saved: '已保存', unconverted: '未转换' };
  const EMPTY: Record<ListFilter, string> = {
    all: '资料库是空的。点“导入文章”添加第一篇英文文章。',
    converting: '没有正在转换的文章。',
    saved: '保存的文章会一直留在本机，不会被缓存清理。',
    unconverted: '所有文章都已转换。',
  };

  const sorted = $derived([...store.articles].sort((a, b) => b.created_at_ms - a.created_at_ms));
  const q = $derived(store.search.trim().toLowerCase());
  const items = $derived.by(() => {
    if (search) {
      if (!q) return [];
      return sorted.filter((a) => a.title.toLowerCase().includes(q) || (store.details[a.id]?.body ?? '').toLowerCase().includes(q));
    }
    switch (filter) {
      case 'converting':
        return sorted.filter(isConverting);
      case 'saved':
        return sorted.filter((a) => a.pinned);
      case 'unconverted':
        return sorted.filter((a) => a.status === 'not_cached');
      default:
        return sorted;
    }
  });

</script>

<div class="row head">
  <h1 class="h1">{search ? `搜索“${store.search.trim()}”` : TITLES[filter]}</h1>
  <button class="play row imp" onclick={() => (store.importOpen = true)}><Icon name="plus" size={16} stroke={2.4} />导入文章</button>
</div>


{#if items.length}
  <ArticleTable {items} query={search ? store.search : ''} />
{:else if store.loaded}
  <div class="empty">{search ? '没有找到匹配的文章。' : EMPTY[filter]}</div>
{/if}

<style>
  .head {
    justify-content: space-between;
    gap: 16px;
  }
  .head h1 {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .imp {
    flex-shrink: 0;
    height: 40px;
    padding: 0 18px;
    gap: 8px;
    font-weight: 700;
    font-size: 14px;
  }
  .empty {
    padding: 60px 12px;
    color: var(--t2);
    font-size: 14px;
  }
</style>
