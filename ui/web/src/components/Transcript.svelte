<script lang="ts">
  // 右侧原文面板 460（UI_SPEC §5 / §3.4）：当前句卡片 + 逐词高亮 + 跟读标注；当前句固定在约 35% 高度；
  // 手动滚动后暂停自动跟随，3 秒后或点“回到当前句”恢复。
  import { untrack } from 'svelte';
  import type { SentenceSpan } from '../lib/types';
  import { store } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import ReadingSettings from './ReadingSettings.svelte';

  type Seg = { kind: 'word'; i: number; text: string } | { kind: 'gap'; text: string; link: boolean };

  const enc = new TextEncoder();
  const dec = new TextDecoder();
  const segCache = new WeakMap<SentenceSpan, Seg[]>();

  /** 按词的字节区间把原句切成“词 / 间隔”，保留标点；连读标在两词之间的间隔上。 */
  function segments(s: SentenceSpan): Seg[] {
    let out = segCache.get(s);
    if (out) return out;
    out = [];
    const bytes = enc.encode(s.text);
    let prev = 0;
    let ok = s.words.length > 0;
    for (const w of s.words) {
      if (w.char_start < prev || w.char_end > bytes.length || w.char_end < w.char_start) ok = false;
      prev = w.char_end;
    }
    if (!ok) {
      s.words.forEach((w, i) => {
        if (i) out!.push({ kind: 'gap', text: ' ', link: !!s.words[i - 1].notes?.link });
        out!.push({ kind: 'word', i, text: w.text });
      });
    } else {
      prev = 0;
      s.words.forEach((w, i) => {
        const gap = dec.decode(bytes.slice(prev, w.char_start));
        const link = i > 0 && !!s.words[i - 1].notes?.link;
        if (gap || link) out!.push({ kind: 'gap', text: gap, link });
        out!.push({ kind: 'word', i, text: dec.decode(bytes.slice(w.char_start, w.char_end)) });
        prev = w.char_end;
      });
      const tail = dec.decode(bytes.slice(prev));
      if (tail) out.push({ kind: 'gap', text: tail, link: false });
    }
    segCache.set(s, out);
    return out;
  }

  const t = $derived(store.timeline);
  const cur = $derived(store.located.sentence);
  const word = $derived(store.located.word);
  const notes = $derived(store.prefs.showNotes);
  const curS = $derived(t && cur != null ? t.sentences[cur] : null);
  const stats = $derived.by(() => {
    if (!curS) return { links: 0, weak: 0 };
    let links = 0;
    let weak = 0;
    curS.words.forEach((w, i) => {
      if (w.notes?.link && i < curS.words.length - 1) links++;
      if (w.notes?.weak_form) weak++;
    });
    return { links, weak };
  });

  function wordClass(s: SentenceSpan, i: number): string {
    const c: string[] = [word == null || i > word ? 'up' : i === word ? 'now' : 'read'];
    if (notes) {
      const n = s.words[i].notes;
      const prev = i > 0 ? s.words[i - 1].notes : undefined;
      if (n?.weak_form) c.push('weak');
      if (n?.flap || n?.elided || prev?.link === 'flap' || prev?.link === 'assimilation') c.push('dot');
    }
    return c.join(' ');
  }

  // ---------- 自动跟随 ----------
  let scroller = $state<HTMLDivElement>();
  let follow = $state(true);
  // 顶部留白 = 可视高度的 35%：开头几句也能停在 35% 高度
  let padTop = $state(40);
  let padBottom = $state(400);
  let resumeTimer = 0;
  let anim = 0;

  function userScrolled() {
    if (!t) return;
    follow = false;
    cancelAnimationFrame(anim);
    clearTimeout(resumeTimer);
    resumeTimer = window.setTimeout(() => {
      follow = true;
      scrollToCurrent();
    }, 3000);
  }

  function scrollToCurrent(instant = false) {
    if (!scroller || cur == null) return;
    const el = scroller.querySelector<HTMLElement>(`[data-i="${cur}"]`);
    if (!el) return;
    const target = Math.max(0, el.offsetTop - scroller.clientHeight * 0.35);
    cancelAnimationFrame(anim);
    const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (instant || reduce) {
      scroller.scrollTop = target;
      return;
    }
    const from = scroller.scrollTop;
    const t0 = performance.now();
    const D = 300;
    const step = (now: number) => {
      const p = Math.min(1, (now - t0) / D);
      const e = 1 - Math.pow(1 - p, 3);
      scroller!.scrollTop = from + (target - from) * e;
      if (p < 1) anim = requestAnimationFrame(step);
    };
    anim = requestAnimationFrame(step);
  }

  $effect(() => {
    void cur;
    if (untrack(() => follow)) untrack(() => requestAnimationFrame(() => scrollToCurrent()));
  });

  $effect(() => {
    // 首次挂载 / 换文章 / 字号变化时直接定位
    void t?.sentences.length;
    void store.player.article_id;
    void store.prefs.fontSize;
    void store.prefs.lineHeight;
    void padTop;
    untrack(() => requestAnimationFrame(() => scrollToCurrent(true)));
  });

  $effect(() => {
    if (!scroller) return;
    const ro = new ResizeObserver(() => {
      const h = scroller!.clientHeight;
      padTop = Math.max(40, Math.round(h * 0.35));
      padBottom = Math.round(h * 0.65);
    });
    ro.observe(scroller);
    return () => ro.disconnect();
  });

  function backToCurrent() {
    clearTimeout(resumeTimer);
    follow = true;
    scrollToCurrent();
  }

  function clickSentence(e: MouseEvent, s: SentenceSpan) {
    if (!s.ready) return;
    const wEl = (e.target as HTMLElement).closest<HTMLElement>('[data-w]');
    if (wEl && s.index === cur) {
      store.popover = {
        sentence: s,
        wordIndex: Number(wEl.dataset.w),
        rect: wEl.getBoundingClientRect(),
        box: wEl.closest('.card')?.getBoundingClientRect(),
      };
      e.stopPropagation();
      return;
    }
    store.popover = null;
    clearTimeout(resumeTimer);
    follow = true;
    store.seekSentence(s.index);
  }

  let aaOpen = $state(false);
</script>

<aside class="tp fade-panel" aria-label="原文">
  <i class="split" style="left:0"></i>
  <div class="row head">
      <div class="label">
        原文{#if t && cur != null}&nbsp;· 第 {cur + 1} / {t.sentences.length} 句{/if}
      </div>
      <div class="row" style="gap:8px;position:relative">
        <button class="chip muted" class:on={aaOpen} aria-label="阅读设置" aria-expanded={aaOpen} onclick={() => (aaOpen = !aaOpen)}>
          <Icon name="textsize" size={14} stroke={1.9} />Aa
        </button>
        <button class="chip {notes ? 'violet' : 'muted'}" aria-pressed={notes} onclick={() => store.toggleNotes()}>
          <Icon name="annot" size={14} stroke={2.2} />跟读标注
        </button>
        {#if aaOpen}
          <div class="aa popup" role="dialog" aria-label="阅读设置">
            <div class="row" style="justify-content:space-between;margin-bottom:14px">
              <div class="h2" style="font-size:17px">阅读设置</div>
              <button class="icon-btn" style="width:32px;height:32px" aria-label="关闭" onclick={() => (aaOpen = false)}
                ><Icon name="close" size={16} stroke={2} /></button
              >
            </div>
            <ReadingSettings compact />
          </div>
        {/if}
      </div>
  </div>
  <div class="inner fade-under">
    {#if !t}
      <div class="empty">
        {store.player.article_id ? '正在载入原文…' : '播放一篇文章，原文会在这里逐词同步显示。'}
      </div>
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="scroller"
        bind:this={scroller}
        onwheel={userScrolled}
        ontouchmove={userScrolled}
        onpointerdown={(e) => e.target === scroller && userScrolled()}
      >
        <div class="spacer" style:height="{padTop - 18}px" aria-hidden="true"></div>
        {#each t.sentences as s (s.index)}
          {@const para = s.index > 0 && s.paragraph !== t.sentences[s.index - 1].paragraph}
          {#if s.index === cur}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div class="card" class:para data-i={s.index} onclick={(e) => clickSentence(e, s)}>
              <div class="tx cur">
                {#if !s.words.length}{s.text}{:else}{#each segments(s) as seg}{#if seg.kind === 'gap'}{#if notes && seg.link && !seg.text.trim()}<span
                          class="lk">‿</span
                        >{:else}{seg.text}{#if notes && seg.link}<span class="lk">‿</span>{/if}{/if}{:else}<span
                        class="w {wordClass(s, seg.i)}"
                        data-w={seg.i}>{seg.text}</span
                      >{/if}{/each}{/if}
              </div>
              {#if notes && (stats.links || stats.weak)}
                <div class="row chips">
                  {#if stats.links}<span class="chip violet">{stats.links} 处连读</span>{/if}
                  {#if stats.weak}<span class="chip muted">{stats.weak} 个弱读</span>{/if}
                </div>
              {/if}
            </div>
          {:else}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div
              class="tx oth {s.ready ? 'sent-other' : 'sent-pending'}"
              class:para
              data-i={s.index}
              title={s.ready ? undefined : '尚未转换'}
              onclick={(e) => clickSentence(e, s)}
            >
              {s.text}
            </div>
          {/if}
        {/each}
        <div class="spacer" style:height="{padBottom}px" aria-hidden="true"></div>
      </div>
    {/if}
  </div>
  {#if t && !follow}
    <button class="chip neutral back" onclick={backToCurrent}>回到当前句</button>
  {/if}
</aside>

<style>
  .tp {
    position: relative;
    width: 460px;
    flex-shrink: 0;
    padding: 24px 0 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .inner {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .head {
    justify-content: space-between;
    padding: 0 28px;
    min-height: 30px;
    position: relative;
    z-index: 2;
  }
  .chip.muted:hover,
  .chip.muted.on {
    color: var(--t1);
  }
  .aa {
    position: absolute;
    top: 40px;
    right: 0;
    width: 380px;
    padding: 16px 18px 18px;
    z-index: 5;
  }
  .scroller {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 28px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    scrollbar-width: none;
  }
  .scroller::-webkit-scrollbar {
    display: none;
  }
  .oth {
    font-size: var(--tx-oth);
    line-height: var(--tx-oth-lh);
    cursor: pointer;
    border-radius: 10px;
    transition: color 0.25s;
  }
  .oth.sent-other:hover {
    color: var(--t2);
  }
  .oth.sent-pending {
    cursor: default;
  }
  .spacer {
    flex-shrink: 0;
  }
  .para {
    margin-top: 12px;
  }
  .card {
    padding: 16px 18px;
    border-radius: 18px;
    background: var(--card);
    box-shadow:
      inset 0 0 0 1px var(--accent-soft),
      var(--elev);
  }
  .cur {
    font-size: var(--tx-cur);
    line-height: var(--tx-cur-lh);
  }
  .w {
    cursor: pointer;
    transition:
      color 0.2s,
      background-color 0.2s;
  }
  .w.up:hover,
  .w.read:hover {
    color: var(--t1);
    background: var(--glass-edge);
    border-radius: 6px;
  }
  .chips {
    gap: 8px;
    margin-top: 12px;
  }
  .chips .chip {
    height: 26px;
    font-size: 12px;
  }
  .empty {
    padding: 60px 28px;
    color: var(--t2);
    font-size: 14px;
    line-height: 22px;
  }
  .back {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    bottom: 110px;
    height: 34px;
    box-shadow: var(--glass-shadow);
    z-index: 3;
  }
</style>
