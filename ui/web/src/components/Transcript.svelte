<script lang="ts">
  // 原文（UI_SPEC §5 / §3.4）：当前句卡片 + 逐词高亮 + 跟读标注；当前句固定在约 35% 高度；
  // 每一句的字号、字重、跟读标注与内边距都相同，当前句只换颜色和底色——切句时排版不跳动；
  // 手动滚动后暂停自动跟随，3 秒后或点“回到当前句”恢复。放在播放页右栏（透明底、字号放大）。
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

  /** 非当前句不读 `word`：逐词推进时只有当前句的词需要重算。 */
  function wordClass(s: SentenceSpan, i: number, on: boolean): string {
    const c: string[] = on ? [word == null || i > word ? 'up' : i === word ? 'now' : 'read'] : [];
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
        box: wEl.closest('.line')?.getBoundingClientRect(),
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

<aside class="tp" aria-label="原文">
  <div class="row head">
      <div class="label">
        原文{#if t && cur != null}&nbsp;· 第 {cur + 1} / {t.sentences.length} 句{/if}{#if notes && stats.links}<span
            class="st violet">&nbsp;· {stats.links} 处连读</span
          >{/if}{#if notes && stats.weak}&nbsp;· {stats.weak} 个弱读{/if}
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
        <div class="spacer" style:height="{padTop - 4}px" aria-hidden="true"></div>
        {#each t.sentences as s (s.index)}
          {@const para = s.index > 0 && s.paragraph !== t.sentences[s.index - 1].paragraph}
          {@const on = s.index === cur}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div
            class="line tx"
            class:on
            class:pending={!s.ready}
            class:para
            data-i={s.index}
            title={s.ready ? undefined : '尚未转换'}
            onclick={(e) => clickSentence(e, s)}
          >
            {#if !s.words.length}{s.text}{:else}{#each segments(s) as seg}{#if seg.kind === 'gap'}{#if notes && seg.link && !seg.text.trim()}<span
                      class="lk">‿</span
                    >{:else}{seg.text}{#if notes && seg.link}<span class="lk">‿</span>{/if}{/if}{:else}<span
                    class="w {wordClass(s, seg.i, on)}"
                    data-w={seg.i}>{seg.text}</span
                  >{/if}{/each}{/if}
          </div>
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
  /* 铺满播放页右栏、透明底 */
  .tp {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
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
    gap: 4px;
    scrollbar-width: none;
    /* 顶部渐隐：滚上去的句子在标题行下方淡出 */
    -webkit-mask-image: linear-gradient(to bottom, transparent, #000 48px);
    mask-image: linear-gradient(to bottom, transparent, #000 48px);
  }
  .scroller::-webkit-scrollbar {
    display: none;
  }
  .spacer {
    flex-shrink: 0;
  }
  /* 每一句同字号、同字重、同内边距：当前句只换颜色与卡片底，换句时文字不重排。
     字号 = 阅读设置的当前句字号 × 1.1（播放页留白大，略放大）。 */
  .line {
    flex-shrink: 0;
    font-size: calc(var(--tx-cur) * 1.1);
    line-height: calc(var(--tx-cur-lh) * 1.1);
    padding: 14px 18px;
    margin: 0 -18px;
    border-radius: 22px;
    color: var(--other);
    cursor: pointer;
    transition:
      color 0.3s,
      background-color 0.3s,
      box-shadow 0.3s;
  }
  .line:not(.on):not(.pending):hover {
    color: var(--t2);
  }
  .line.pending {
    color: var(--pending);
    cursor: default;
  }
  .line.on {
    color: var(--t1);
    background: var(--card);
    box-shadow:
      inset 0 0 0 1px var(--accent-soft),
      var(--elev);
  }
  /* 非当前句的跟读标注跟着整句调暗 */
  .line:not(.on) :global(.lk) {
    opacity: 0.5;
  }
  .line:not(.on) :global(.dot) {
    border-bottom-color: color-mix(in srgb, var(--violet) 40%, transparent);
  }
  .line:not(.on) :global(.weak) {
    color: inherit;
  }
  .para {
    margin-top: 12px;
  }
  .st {
    color: var(--violet-text);
  }
  .w {
    transition:
      color 0.2s,
      background-color 0.2s,
      box-shadow 0.2s;
  }
  .on .w {
    cursor: pointer;
  }
  .on .w.up:hover,
  .on .w.read:hover {
    color: var(--t1);
    background: var(--glass-edge);
    box-shadow: 0 0 0 3px var(--glass-edge);
    border-radius: 6px;
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
    bottom: 40px;
    height: 34px;
    box-shadow: var(--glass-shadow);
    z-index: 3;
  }
</style>
