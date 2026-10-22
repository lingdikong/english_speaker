<script lang="ts">
  // 点词读音（UI_SPEC §4.5 / §5）：词旁 popover。词组（‿ violet）+ IPA + 发音；类型 chip + 美式/英式；中文说明；“同类”示例；
  // “从这里播放” / “0.75× 慢放这句”。
  import { store } from '../lib/store.svelte';
  import type { Link, WordSpan } from '../lib/types';
  import Icon from './Icon.svelte';

  const LINKS: Record<Link, { name: string; desc: (a: string, b: string) => string; same: string[][] }> = {
    consonant_vowel: {
      name: '辅音连元音',
      desc: (a, b) => `${a} 词尾的辅音直接带到 ${b} 开头的元音上，两个词连成一个音节读出。`,
      same: [['pick', 'it'], ['turn', 'off'], ['a lot', 'of']],
    },
    glide_j: {
      name: '元音连读 /j/',
      desc: (a, b) => `${a} 以 /iː/ 一类元音结尾、${b} 以元音开头，中间自然带出一个轻轻的 /j/。`,
      same: [['see', 'it'], ['I', 'am'], ['the', 'end']],
    },
    glide_w: {
      name: '元音连读 /w/',
      desc: (a, b) => `${a} 以 /uː/ /oʊ/ 一类元音结尾、${b} 以元音开头，中间自然带出一个轻轻的 /w/。`,
      same: [['go', 'on'], ['do', 'it'], ['how', 'about']],
    },
    linking_r: {
      name: '连接 r',
      desc: (a, b) => `${a} 词尾的 r 平时不发音（英式），但后面的 ${b} 以元音开头时要把 /r/ 读出来。`,
      same: [['far', 'away'], ['for', 'example'], ['after', 'all']],
    },
    same_sound: {
      name: '同音连读',
      desc: (a, b) => `${a} 的结尾和 ${b} 的开头是同一个辅音，只发一次，稍微延长。`,
      same: [['big', 'game'], ['bad', 'day'], ['some', 'more']],
    },
    flap: {
      name: '跨词闪音',
      desc: (a, b) => `${a} 末尾的 t 夹在两个元音之间，读得像轻快的 d，并和后面的 ${b} 连成一个音节。`,
      same: [['get', 'it'], ['what', 'are'], ['a lot', 'of']],
    },
    assimilation: {
      name: '同化',
      desc: (a, b) => `${a} 词尾的 t/d 遇到 ${b} 开头的 /j/，融合成 /ʧ/ 或 /ʤ/。`,
      same: [['did', 'you'], ['meet', 'you'], ['would', 'you']],
    },
  };

  const p = $derived(store.popover);
  const s = $derived(p?.sentence ?? null);
  const w = $derived<WordSpan | null>(p && s ? s.words[p.wordIndex] : null);
  const next = $derived<WordSpan | null>(p && s && w?.notes?.link ? (s.words[p.wordIndex + 1] ?? null) : null);
  const clean = (t: string) => t.replace(/^[^\p{L}\p{N}']+|[^\p{L}\p{N}']+$/gu, '');
  const british = $derived(store.voiceOf(store.current)?.accent === 'british');

  const info = $derived.by(() => {
    if (!w) return null;
    const n = w.notes;
    const a = clean(w.text);
    if (n?.link && next) {
      const L = LINKS[n.link];
      const b = clean(next.text);
      return {
        type: L.name,
        ipa: [n.ipa, next.notes?.ipa].filter(Boolean).join(' '),
        desc: L.desc(a, b),
        same: L.same,
      };
    }
    if (n?.weak_form) {
      return {
        type: '弱读',
        ipa: n.weak_form,
        desc: `${a} 是虚词，在句中通常轻读成 /${n.weak_form}/${n.ipa ? `，而不是单独读时的 /${n.ipa}/` : ''}。`,
        same: null,
      };
    }
    if (n?.flap) {
      return { type: '闪音', ipa: n.ipa, desc: `${a} 词中的 t 夹在元音之间，美式读得像轻快的 d。`, same: null };
    }
    if (n?.elided) {
      return { type: '省音', ipa: n.ipa, desc: `${a} 词尾的 t/d 在辅音前通常省略或只做口型、不爆破。`, same: null };
    }
    return { type: null, ipa: n?.ipa ?? '', desc: n?.ipa ? '' : '该词暂无读音标注。', same: null };
  });

  let el = $state<HTMLDivElement>();
  let pos = $state({ x: -9999, y: 0 });
  $effect(() => {
    if (!p || !el) return;
    void info;
    const r = el.getBoundingClientRect();
    const gap = 10;
    const b = p.box ?? p.rect;
    const limit = innerHeight - 92; // 播放条上方
    let y: number;
    if (b.bottom + gap + r.height < limit) y = b.bottom + gap;
    else if (b.top - gap - r.height >= 12) y = b.top - gap - r.height;
    else y = p.rect.bottom + gap + r.height < limit ? p.rect.bottom + gap : Math.max(12, p.rect.top - r.height - gap);
    pos = {
      x: Math.min(innerWidth - r.width - 12, Math.max(12, p.rect.left + p.rect.width / 2 - r.width / 2)),
      y,
    };
  });

  function close() {
    store.popover = null;
  }
</script>

<svelte:window
  onmousedown={(e) => {
    if (store.popover && !(e.target as HTMLElement).closest('.wpop, [data-w]')) close();
  }}
  onresize={close}
/>

{#if p && w && info}
  <div class="wpop popup" bind:this={el} style:left="{pos.x}px" style:top="{pos.y}px" role="dialog" aria-label="读音">
    <div class="row top">
      <div style="min-width:0">
        <div class="phrase">
          {clean(w.text)}{#if next}<span style="color:var(--violet)">‿</span>{clean(next.text)}{/if}
        </div>
        {#if info.ipa}<div class="ipa">/{info.ipa}/</div>{/if}
      </div>
      <button
        class="icon-btn fill"
        aria-label="播放这个词"
        onclick={() => store.playSpan(w.start_ms, (next ?? w).end_ms)}><Icon name="vol" size={20} stroke={1.9} /></button
      >
    </div>
    <div class="row" style="gap:8px">
      {#if info.type}<span class="chip violet">{info.type}</span>{/if}
      <span class="chip muted">{british ? '英式发音' : '美式发音'}</span>
    </div>
    {#if info.desc}<div class="desc">{info.desc}</div>{/if}
    {#if info.same}
      <div class="same">
        同类：{#each info.same as [x, y], i}{#if i}&nbsp;·&nbsp;{/if}<span class="tx" style="color:var(--t1)">{x}<span class="lk">‿</span>{y}</span>{/each}
      </div>
    {/if}
    <div class="row" style="gap:10px">
      <button
        class="play btn"
        onclick={() => {
          store.seek(w.start_ms);
          if (!store.active) store.toggle();
          close();
        }}>从这里播放</button
      >
      <button
        class="chip neutral btn"
        disabled={!store.host.rate_supported}
        onclick={() => {
          store.slowSentence(s!.index);
          close();
        }}>0.75× 慢放这句</button
      >
    </div>
  </div>
{/if}

<style>
  .wpop {
    position: fixed;
    z-index: 60;
    width: 360px;
    padding: 18px 18px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    border-radius: 22px;
  }
  .top {
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
  }
  .phrase {
    font-family: var(--display);
    font-weight: 700;
    font-size: 30px;
    line-height: 36px;
    letter-spacing: -0.02em;
  }
  .ipa {
    font-family: var(--ipa);
    font-size: 18px;
    color: var(--violet-text);
    margin-top: 4px;
  }
  .desc {
    font-size: 14px;
    line-height: 23px;
    color: var(--t2);
  }
  .same {
    padding: 12px 14px;
    border-radius: 16px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: 13px;
    color: var(--t2);
    line-height: 22px;
  }
  .btn {
    flex: 1;
    height: 44px;
    justify-content: center;
    font-weight: 700;
    font-size: 14px;
  }
  .chip.btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
