<script lang="ts">
  // 播放页：点播放条封面打开，铺满整个窗口（参考 Apple Music 的全屏播放，用本应用的令牌与声波语言）。
  // 左栏：大封面（暂停时缩小）· 标题 · 声波进度 · 播放控制 · 音量；右栏：原文逐词同步。
  // 开合动画：从播放条封面处的一颗 accent 小球展开成整个画面（clip-path 圆形），收起时缩回小球。
  import { cubicInOut, quartOut } from 'svelte/easing';
  import { dur } from '../lib/motion';
  import { store, fmtTime } from '../lib/store.svelte';
  import { G } from '../lib/icons';
  import Thumb from './Thumb.svelte';
  import WaveProgress from './WaveProgress.svelte';
  import Transcript from './Transcript.svelte';
  import UpNext from './UpNext.svelte';
  import Icon from './Icon.svelte';

  const a = $derived(store.current);
  const t = $derived(store.timeline);
  const cur = $derived(store.located.sentence);
  const sub = $derived(
    [store.voiceText(a) === '—' ? '' : store.voiceText(a), t && cur != null ? `第 ${cur + 1}/${t.sentences.length} 句` : '']
      .filter(Boolean)
      .join(' · '),
  );
  const remain = $derived(Math.max(0, store.duration - store.position));
  const rate = $derived(store.player.rate);
  const rateText = $derived(`${Math.abs(rate - Math.round(rate * 10) / 10) < 0.001 ? rate.toFixed(1) : rate.toFixed(2)}×`);
  const vol = $derived(store.host.volume);

  // 封面边长随窗口变化：左栏宽度与高度（留出标题、进度、控制约 330px）共同约束
  let w = $state(window.innerWidth);
  let h = $state(window.innerHeight);
  const art = $derived(Math.round(Math.max(180, Math.min(380, w * 0.3, h - 360))));

  // 开合动画拆成三条独立的 CSS 动画，时长与缓动相同：
  //   ball    —— 圆形裁剪从小球半径长到覆盖整个窗口的半径；
  //   orbFade —— 起初是实心 accent 小球，展开到约 45% 时淡尽；
  //   rise    —— 内容在后半程淡入并轻微上浮。
  // 后两条只动 opacity / transform，由合成器执行；不再用 CSS 变量驱动子元素（那会让整页每帧重算样式并重绘）。
  type T = { duration: number; easing: (t: number) => number };

  function ball(node: HTMLElement, { duration, easing }: T) {
    const o = store.playerOrigin;
    const R = Math.hypot(Math.max(o.x, innerWidth - o.x), Math.max(o.y, innerHeight - o.y));
    return {
      duration,
      easing,
      css: (k: number) => `clip-path: circle(${o.r + (R - o.r) * k}px at ${o.x}px ${o.y}px);`,
    };
  }

  function orbFade(node: HTMLElement, { duration, easing }: T) {
    const o = store.playerOrigin;
    node.style.background = `radial-gradient(circle at ${o.x}px ${o.y}px, var(--accent), var(--violet) 70vmax)`;
    return { duration, easing, css: (k: number) => `opacity: ${Math.max(0, 1 - k * 2.2)};` };
  }

  function rise(node: HTMLElement, { duration, easing }: T) {
    return {
      duration,
      easing,
      css: (k: number) => {
        const a = Math.min(1, Math.max(0, (k - 0.3) * 2.4));
        return `opacity: ${a}; transform: translateY(${(1 - k) * 28}px) scale(${0.97 + k * 0.03});`;
      },
    };
  }

  const IN = { duration: dur(620), easing: quartOut };
  const OUT = { duration: dur(440), easing: cubicInOut };
</script>

<svelte:window bind:innerWidth={w} bind:innerHeight={h} />

<div
  class="player"
  role="dialog"
  aria-modal="true"
  aria-label="正在播放"
  in:ball={IN}
  out:ball={OUT}
>
  <div class="bg" class:live={store.player.status === 'playing'} aria-hidden="true"><i class="b1"></i><i class="b2"></i><i class="b3"></i></div>
  <i class="orb" aria-hidden="true" in:orbFade={IN} out:orbFade={OUT}></i>

  <div class="content" in:rise={IN} out:rise={OUT}>
    <header class="row hd">
      <button class="icon-btn fill" aria-label="收起播放页" title="收起（Esc）" onclick={() => store.closePlayer()}>
        <Icon name="down" size={20} stroke={2.2} />
      </button>
      <button
        class="icon-btn"
        class:on={store.queueOpen}
        data-queue-btn
        aria-label="待播清单"
        aria-expanded={store.queueOpen}
        title="待播清单"
        onclick={() => (store.queueOpen = !store.queueOpen)}>{@html G.queue}</button
      >
    </header>

    <div class="body">
      <section class="left">
        <div class="art" class:rest={!store.active} style:width="{art}px" style:height="{art}px">
          {#if a}<Thumb title={a.title} size={art} />{/if}
        </div>

        <div class="meta" style:width="{Math.max(art, 320)}px">
          <div class="ttl" title={a?.title}>{a?.title ?? '未在播放'}</div>
          <div class="sub">{sub || ' '}</div>

          <div class="prog">
            <WaveProgress n={64} height={30} min={4} gap={3} />
            <div class="row times">
              <span>{fmtTime(store.position)}</span>
              <span>{store.player.duration_final ? '' : '约 '}-{fmtTime(remain)}</span>
            </div>
          </div>

          <div class="row ctrls">
            <button
              class="cbtn"
              class:on={store.loop}
              aria-label={store.loop ? '单句循环（已开启）' : '单句循环'}
              aria-pressed={store.loop}
              title="单句循环（L）"
              onclick={() => store.toggleLoop()}>{@html G.loop}</button
            >
            <button class="cbtn big" aria-label="上一句" title="上一句（⌘/Ctrl + ←）" disabled={!a} onclick={() => store.prevSentence()}
              >{@html G.prev}</button
            >
            <button
              class="cbtn main"
              aria-label={store.player.status === 'buffering' ? '缓冲中' : store.active ? '暂停' : '播放'}
              title="播放 / 暂停（空格）"
              onclick={() => store.toggle()}
            >
              {#if store.player.status === 'buffering'}<span class="spinner" aria-hidden="true"></span>
              {:else}{@html store.active ? G.pause : G.play}{/if}
            </button>
            <button class="cbtn big" aria-label="下一句" title="下一句（⌘/Ctrl + →）" disabled={!a} onclick={() => store.nextSentence()}
              >{@html G.next}</button
            >
            <button class="cbtn rate" aria-label="倍速 {rateText}" title="倍速" onclick={() => store.cycleRate()}>{rateText}</button>
          </div>

          <div class="row volume">
            <button class="vbtn" aria-label={vol <= 0.001 ? '取消静音' : '静音'} onclick={() => store.setVolume(vol <= 0.001 ? 1 : 0)}
              >{@html G.mute}</button
            >
            <input
              class="slider"
              type="range"
              min="0"
              max="1"
              step="0.01"
              value={vol}
              style:--fill="{vol * 100}%"
              aria-label="音量"
              oninput={(e) => store.setVolume(Number((e.target as HTMLInputElement).value))}
            />
            <span class="vbtn" aria-hidden="true">{@html G.vol}</span>
          </div>
        </div>
      </section>

      <section class="right">
        <Transcript />
      </section>
    </div>
  </div>

  {#if store.queueOpen}<UpNext inPlayer />{/if}
</div>

<style>
  .player {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: var(--bg);
    overflow: hidden;
  }

  /* 背景：两三团极淡的 accent / violet 光晕，播放时缓慢漂移（径向渐变，不用 blur，便宜） */
  .bg {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .bg i {
    position: absolute;
    border-radius: 50%;
    animation: drift 22s ease-in-out infinite alternate;
    animation-play-state: paused;
  }
  .bg.live i {
    animation-play-state: running;
  }
  .b1 {
    width: 90vmax;
    height: 90vmax;
    left: -35vmax;
    top: -40vmax;
    background: radial-gradient(circle, var(--accent-soft), transparent 62%);
  }
  .bg .b2 {
    width: 80vmax;
    height: 80vmax;
    right: -30vmax;
    bottom: -45vmax;
    background: radial-gradient(circle, var(--violet-soft), transparent 62%);
    animation-duration: 28s;
    animation-direction: alternate-reverse;
  }
  .bg .b3 {
    width: 50vmax;
    height: 50vmax;
    left: 20vw;
    bottom: -30vmax;
    background: radial-gradient(circle, var(--accent-soft), transparent 65%);
    opacity: 0.7;
    animation-duration: 34s;
  }
  @keyframes drift {
    from {
      transform: translate(0, 0) scale(1);
    }
    to {
      transform: translate(6vmax, 4vmax) scale(1.12);
    }
  }

  /* 开合时的小球（底色由 orbFade 按小球位置设置），平时完全透明 */
  .orb {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
  }

  .content {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  /* 顶部 44px 让给窗口拖动区与窗口按钮（macOS 红绿灯 / Windows 右上角） */
  .hd {
    justify-content: space-between;
    padding: 44px 32px 0;
  }
  .icon-btn.on {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(340px, 0.85fr) minmax(0, 1fr);
    gap: 48px;
    padding: 0 48px 0 56px;
  }
  .left {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 28px;
    padding-bottom: 48px;
    min-width: 0;
  }
  .right {
    display: flex;
    min-width: 0;
    min-height: 0;
    padding-top: 8px;
  }

  /* 大封面：播放时满尺寸、暂停时缩小（同 Apple Music），弹簧缓动 */
  .art {
    flex-shrink: 0;
    border-radius: 22%;
    box-shadow: var(--elev);
    transition: transform 0.6s var(--spring), box-shadow 0.6s;
  }
  .art :global(.cover) {
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .art.rest {
    transform: scale(0.84);
    box-shadow: none;
  }

  .meta {
    max-width: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .ttl {
    font-family: var(--display);
    font-weight: 700;
    font-size: 24px;
    line-height: 30px;
    letter-spacing: -0.02em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    font-size: 13px;
    font-weight: 600;
    color: var(--t2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .prog {
    margin-top: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .prog :global(.wp) {
    flex: none;
    width: 100%;
  }
  .times {
    justify-content: space-between;
    font-size: 12px;
    font-weight: 700;
    color: var(--t2);
    font-variant-numeric: tabular-nums;
  }

  .ctrls {
    justify-content: space-between;
    margin-top: 10px;
  }
  .cbtn {
    width: 48px;
    height: 48px;
    border-radius: 999px;
    border: none;
    padding: 0;
    background: transparent;
    color: var(--t1);
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      background-color 0.2s,
      color 0.2s,
      transform 0.25s var(--spring);
  }
  .cbtn:hover:not(:disabled) {
    background: var(--glass-edge);
  }
  .cbtn:active:not(:disabled) {
    transform: scale(0.92);
  }
  .cbtn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .cbtn.on {
    color: var(--accent-text);
  }
  .cbtn.big :global(svg) {
    width: 32px;
    height: 32px;
  }
  .cbtn.main {
    width: 72px;
    height: 72px;
    background: var(--accent);
    color: var(--accent-ink);
    box-shadow: 0 10px 28px color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .cbtn.main:hover {
    background: var(--accent);
    filter: brightness(1.06);
  }
  .cbtn.main :global(svg) {
    width: 34px;
    height: 34px;
  }
  .cbtn.main .spinner {
    border-color: color-mix(in srgb, var(--accent-ink) 30%, transparent);
    border-top-color: var(--accent-ink);
  }
  .rate {
    width: auto;
    min-width: 56px;
    padding: 0 10px;
    font-weight: 700;
    font-size: 15px;
  }

  .volume {
    gap: 12px;
    margin-top: 6px;
    color: var(--t2);
  }
  .volume .slider {
    flex: 1;
    min-width: 0;
  }
  .vbtn {
    width: 32px;
    height: 32px;
    padding: 0;
    border: none;
    border-radius: 999px;
    background: none;
    color: inherit;
    display: grid;
    place-items: center;
  }
  .vbtn :global(svg) {
    width: 20px;
    height: 20px;
  }
  button.vbtn:hover {
    color: var(--t1);
  }

  /* 窄窗口：两栏间距收窄 */
  @media (max-width: 1100px) {
    .body {
      gap: 28px;
      padding: 0 28px 0 32px;
    }
  }
</style>
