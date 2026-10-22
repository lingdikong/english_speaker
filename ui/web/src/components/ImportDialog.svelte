<script lang="ts">
  // 导入文章（UI_SPEC §4.6，PC 为居中对话框）：粘贴文本 / 文件 / 剪贴板 · 标题（可选）· 正文 · 音色 + 导入后立即播放 ·
  // “共 N 句 · 预计约 N 秒 · 离线转换，不上传” · 开始转换。确定后立刻出现在资料库并开始合成（边听边转）。
  import { fade, scale } from 'svelte/transition';
  import { dur } from '../lib/motion';
  import { store, voiceLabel } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  type Mode = 'paste' | 'file' | 'clip';
  let mode = $state<Mode>(store.importFile ? 'file' : 'paste');
  let title = $state(store.importFile ? store.importFile.name.replace(/\.[^.]+$/, '') : '');
  let body = $state(store.importFile?.text ?? '');
  let fileName = $state<string | null>(store.importFile?.name ?? null);
  let voice = $state(store.prefs.defaultVoice ?? store.voices[0]?.id ?? '');
  let playNow = $state(true);
  let busy = $state(false);
  let error = $state('');
  let over = $state(false);
  let fileInput = $state<HTMLInputElement>();
  let textarea = $state<HTMLTextAreaElement>();

  $effect(() => {
    if (mode === 'paste') textarea?.focus();
    return () => (store.importFile = null);
  });

  /** 与核心 `text::estimate_ms` 同口径的粗略估计（每个字母/数字约 62ms + 句间停顿 300ms）。 */
  const stats = $derived.by(() => {
    const text = body.trim();
    if (!text) return null;
    const sentences = text.split(/(?<=[.!?…])["'”’)\]]*\s+|\n+/).filter((x) => /[A-Za-z0-9]/.test(x)).length;
    const chars = (text.match(/[A-Za-z0-9]/g) ?? []).length;
    const sec = Math.round((chars * 62 + sentences * 300) / 1000);
    return { sentences, time: sec < 90 ? `${Math.max(1, sec)} 秒` : `${Math.round(sec / 60)} 分钟` };
  });

  async function readFile(f: File) {
    if (!/\.(txt|md|markdown|text)$/i.test(f.name)) {
      error = '只支持 .txt / .md 文件';
      return;
    }
    body = await f.text();
    fileName = f.name;
    if (!title.trim()) title = f.name.replace(/\.[^.]+$/, '');
    error = '';
  }

  async function readClipboard() {
    error = '';
    try {
      const text = await navigator.clipboard.readText();
      if (!text.trim()) error = '剪贴板里没有文本';
      else {
        body = text;
        fileName = null;
      }
    } catch {
      error = '无法读取剪贴板：请切换到“粘贴文本”后按 ⌘V / Ctrl+V 粘贴';
    }
  }

  function setMode(m: Mode) {
    mode = m;
    error = '';
    if (m === 'clip') readClipboard();
    if (m === 'paste') fileName = null;
  }

  async function submit() {
    if (!body.trim() || busy) return;
    busy = true;
    error = '';
    try {
      const def = store.voices[0]?.id;
      await store.importText(body, {
        title: title.trim() || undefined,
        sourceRef: mode === 'file' ? (fileName ?? undefined) : undefined,
        voice: voice && voice !== def ? voice : null,
        play: playNow,
      });
      store.importOpen = false;
      if (!playNow) store.view = { kind: 'list', filter: 'all' };
    } catch (e) {
      error = String((e as Error).message ?? e);
    } finally {
      busy = false;
    }
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'Escape') store.importOpen = false;
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) submit();
    e.stopPropagation();
  }
</script>

<div class="scrim" transition:fade={{ duration: dur(180) }} onclick={() => (store.importOpen = false)} role="presentation"></div>
<div class="wrap" role="presentation">
  <div
    class="sheet"
    transition:scale={{ start: 0.96, duration: dur(260), opacity: 0 }}
    role="dialog"
    aria-modal="true"
    aria-label="导入文章"
    tabindex="-1"
    onkeydown={keydown}
  >
    <div class="row hd">
      <button class="icon-btn fill" aria-label="关闭" onclick={() => (store.importOpen = false)}><Icon name="close" size={20} stroke={2} /></button>
      <div class="h2">导入文章</div>
      <div style="width:40px"></div>
    </div>

    <div class="seg modes" role="tablist">
      <button role="tab" aria-selected={mode === 'paste'} class:on={mode === 'paste'} onclick={() => setMode('paste')}>粘贴文本</button>
      <button role="tab" aria-selected={mode === 'file'} class:on={mode === 'file'} onclick={() => setMode('file')}>文件</button>
      <button role="tab" aria-selected={mode === 'clip'} class:on={mode === 'clip'} onclick={() => setMode('clip')}>剪贴板</button>
    </div>

    <label class="fl">
      <span class="label">标题（可选）</span>
      <input class="field" bind:value={title} placeholder="留空时取第一句" spellcheck="false" />
    </label>

    {#if mode === 'file' && !fileName}
      <div
        class="drop"
        class:over
        role="button"
        tabindex="0"
        ondragover={(e) => (e.preventDefault(), (over = true))}
        ondragleave={() => (over = false)}
        ondrop={(e) => {
          e.preventDefault();
          over = false;
          const f = e.dataTransfer?.files[0];
          if (f) readFile(f);
        }}
        onclick={() => fileInput?.click()}
        onkeydown={(e) => e.key === 'Enter' && fileInput?.click()}
      >
        <Icon name="file" size={28} />
        <div><b>选择文件</b> 或拖到这里</div>
        <div class="hint">.txt / .md，UTF-8</div>
      </div>
    {:else}
      <label class="fl grow">
        <span class="label row" style="justify-content:space-between">
          <span>正文</span>
          {#if fileName}<span class="fname">{fileName}</span>{/if}
        </span>
        <textarea
          bind:this={textarea}
          bind:value={body}
          class="field ta"
          placeholder="粘贴英文文章…"
          spellcheck="false"
        ></textarea>
      </label>
    {/if}
    <input
      bind:this={fileInput}
      type="file"
      accept=".txt,.md,.markdown,.text,text/plain,text/markdown"
      hidden
      onchange={(e) => {
        const f = (e.target as HTMLInputElement).files?.[0];
        if (f) readFile(f);
      }}
    />

    <div class="group">
      <div class="item">
        <span>音色</span>
        <select class="chip neutral sel" bind:value={voice} aria-label="音色">
          {#each store.voices as v}<option value={v.id}>{voiceLabel(v)}</option>{/each}
        </select>
      </div>
      <div class="item">
        <span>导入后立即播放</span>
        <button class="switch" role="switch" aria-checked={playNow} aria-label="导入后立即播放" onclick={() => (playNow = !playNow)}></button>
      </div>
    </div>

    {#if error}
      <div class="err">{error}</div>
    {:else}
      <div class="meta">{stats ? `共 ${stats.sentences} 句 · 预计约 ${stats.time} · ` : ''}离线转换，不上传</div>
    {/if}
    <button class="play go" disabled={!body.trim() || busy} onclick={submit}>{busy ? '正在导入…' : '开始转换'}</button>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.45);
    z-index: 70;
  }
  .wrap {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    z-index: 71;
    pointer-events: none;
    padding: 24px;
  }
  .sheet {
    pointer-events: auto;
    width: min(600px, 100%);
    max-height: calc(100vh - 48px);
    background: var(--surface);
    border-radius: 28px;
    box-shadow:
      inset 0 0 0 1px var(--line),
      0 30px 80px rgba(0, 0, 0, 0.35);
    padding: 18px 22px 22px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    outline: none;
  }
  .hd {
    justify-content: space-between;
  }
  .modes > button {
    height: 38px;
  }
  .modes > button.on {
    background: var(--card);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
  }
  .fl {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .grow {
    flex: 1;
    min-height: 0;
  }
  .ta {
    height: auto;
    min-height: 200px;
    max-height: 40vh;
    resize: none;
    padding: 14px;
    line-height: 24px;
    font-weight: 500;
  }
  .fname {
    color: var(--accent-text);
    letter-spacing: 0;
  }
  .drop {
    min-height: 200px;
    border-radius: 14px;
    border: 2px dashed var(--line);
    background: var(--card);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: var(--t2);
    font-size: 14px;
    cursor: pointer;
  }
  .drop b {
    color: var(--accent-text);
  }
  .drop.over {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .hint {
    font-size: 12px;
    color: var(--t3);
  }
  .sel {
    appearance: none;
    -webkit-appearance: none;
    outline: none;
    cursor: pointer;
  }
  .meta {
    font-size: 13px;
    color: var(--t2);
    text-align: center;
  }
  .err {
    font-size: 13px;
    color: var(--danger-text);
    text-align: center;
  }
  .go {
    height: 54px;
    font-weight: 700;
    font-size: 16px;
    flex-shrink: 0;
  }
</style>
