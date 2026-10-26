<script lang="ts">
  // 设置页（UI_SPEC §4.7，PC 两列卡片）：外观 · 阅读 · 翻译 · 朗读 · 本地缓存 · 关于。
  import { store, fmtBytes, voiceLabel, RATES, type Theme } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import ReadingSettings from './ReadingSettings.svelte';
  import TranslationSettings from './TranslationSettings.svelte';

  const GB = 1024 ** 3;
  /** “不限”：核心以字节数表示上限，这里用 JS 能精确表示的最大整数。 */
  const UNLIMITED = Number.MAX_SAFE_INTEGER;
  const LIMITS: [number, string][] = [
    [GB, '1 GB'],
    [2 * GB, '2 GB'],
    [5 * GB, '5 GB'],
    [UNLIMITED, '不限'],
  ];
  const THEMES: [Theme, string, 'sun' | 'moon' | null][] = [
    ['system', '跟随系统', null],
    ['light', '白天', 'sun'],
    ['dark', '夜间', 'moon'],
  ];

  const c = $derived(store.cache);
  const unlimited = $derived((c?.limit_bytes ?? 0) >= 1e15);
  /** 用量条的满格：有上限时 = 上限；不限时 = 已用（至少 1 GB）。 */
  const scale = $derived(c ? (unlimited ? Math.max(c.used_bytes, GB) : Math.max(1, c.limit_bytes)) : 1);
  const pinnedPct = $derived(c ? Math.min(100, (c.pinned_bytes / scale) * 100) : 0);
  const otherPct = $derived(c ? Math.min(100 - pinnedPct, ((c.used_bytes - c.pinned_bytes) / scale) * 100) : 0);
  const defVoice = $derived(store.prefs.defaultVoice ?? store.voices[0]?.id ?? '');

  $effect(() => {
    store.refreshCache();
  });

  function clear() {
    store.confirm = {
      title: '清除未保存的缓存？',
      body: '未保存文章的音频会被删除，再次播放时会重新离线转换。已保存的文章不受影响。',
      ok: '清除',
      danger: true,
      action: () => store.clearCache(),
    };
  }
</script>

<div class="row head"><h1 class="h1">设置</h1></div>

<div class="cols">
  <div class="col">
    <section>
      <div class="label">外观</div>
      <div class="group pad">
        <div class="item">
          <span>主题</span>
          <div class="seg" style="width:320px">
            {#each THEMES as [t, label, icon]}
              <button class:on={store.prefs.theme === t} onclick={() => store.setPref('theme', t)}>
                {#if icon}<Icon name={icon} size={16} />{/if}{label}
              </button>
            {/each}
          </div>
        </div>
      </div>
    </section>

    <section>
      <div class="label">阅读</div>
      <ReadingSettings />
    </section>

    {#if store.translator}
      <section>
        <div class="label">翻译</div>
        <TranslationSettings />
      </section>
    {/if}
  </div>

  <div class="col">
    <section>
      <div class="label">朗读</div>
      <div class="group">
        <div class="item">
          <span>默认音色</span>
          <select
            class="chip neutral sel"
            value={defVoice}
            onchange={(e) => store.setPref('defaultVoice', (e.target as HTMLSelectElement).value)}
            aria-label="默认音色"
          >
            {#each store.voices as v}<option value={v.id}>{voiceLabel(v)}</option>{/each}
          </select>
        </div>
        <div class="item">
          <span>默认倍速</span>
          <div class="seg" style="width:280px">
            {#each RATES as r}
              <button
                class:on={Math.abs(store.prefs.defaultRate - r) < 0.01}
                disabled={!store.host.rate_supported}
                onclick={() => store.setPref('defaultRate', r)}>{r.toFixed(r === 1 ? 1 : 2)}×</button
              >
            {/each}
          </div>
        </div>
      </div>
      <p class="note">新导入的文章使用默认音色；单篇可在列表 ⋯ 中“换音色”。</p>
    </section>

    <section>
      <div class="label">本地缓存</div>
      <div class="group">
        <div class="usage">
          <div class="row" style="justify-content:space-between">
            <span class="big">{c ? fmtBytes(c.used_bytes) : '—'}</span>
            <span class="hint">上限 {c ? (unlimited ? '不限' : fmtBytes(c.limit_bytes)) : '—'} · {c?.entries ?? 0} 篇</span>
          </div>
          <div class="ubar" role="img" aria-label="缓存用量">
            <b class="pin" style:width="{pinnedPct}%"></b><b class="oth" style:width="{otherPct}%"></b>
          </div>
          <div class="row legend">
            <span class="row"><i class="pin"></i>已保存 {c ? fmtBytes(c.pinned_bytes) : '—'}</span>
            <span class="row"><i class="oth"></i>可清理 {c ? fmtBytes(Math.max(0, c.used_bytes - c.pinned_bytes)) : '—'}</span>
          </div>
        </div>
        <div class="item">
          <span>缓存上限</span>
          <div class="seg" style="width:300px">
            {#each LIMITS as [b, label]}
              <button class:on={c != null && (b === UNLIMITED ? unlimited : c.limit_bytes === b)} onclick={() => store.setCacheLimit(b)}
                >{label}</button
              >
            {/each}
          </div>
        </div>
        <div class="item">
          <button class="danger-btn" onclick={clear}>清除未保存的缓存</button>
        </div>
      </div>
      <p class="note">已保存的文章不会被自动清理。超过上限时，最久未听的未保存文章会先被清理。</p>
    </section>

    <section>
      <div class="label">关于</div>
      <div class="group">
        <div class="item"><span>版本</span><span class="hint">English Speaker {__APP_VERSION__}</span></div>
        <div class="item"><span>朗读模型</span><span class="hint">Kokoro-82M v1.0 int8</span></div>
        <div class="item"><span>翻译模型</span><span class="hint">Opus-MT en-zh int8 · ECDICT{#if store.translator?.llm.model_name} · {store.translator.llm.model_name}（可选）{/if}</span></div>
        <div class="item"><span>隐私</span><span class="hint">完全离线，文章不会上传</span></div>
        <div class="item lic">
          <span>开源许可</span>
          <span class="hint"
            >Kokoro-82M（Apache-2.0）· ONNX Runtime（MIT）· misaki 词典（Apache-2.0）· Opus-MT（Apache-2.0）· ECDICT（MIT）·
            Qwen2.5（Apache-2.0）· llama.cpp（MIT）· Space Grotesk / Manrope / Noto Sans（SIL OFL
            1.1）· Svelte（MIT）· CEF（BSD-3-Clause）</span
          >
        </div>
      </div>
    </section>
  </div>
</div>

<style>
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
    gap: 28px 32px;
    align-items: start;
    max-width: 1200px;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 28px;
    min-width: 0;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .group .seg {
    max-width: 60%;
  }
  .sel {
    appearance: none;
    -webkit-appearance: none;
    padding-right: 28px;
    background-image: linear-gradient(45deg, transparent 50%, var(--t2) 50%), linear-gradient(135deg, var(--t2) 50%, transparent 50%);
    background-position:
      calc(100% - 15px) 50%,
      calc(100% - 10px) 50%;
    background-size: 5px 5px;
    background-repeat: no-repeat;
    outline: none;
    cursor: pointer;
    max-width: 60%;
  }
  .note {
    margin: 0;
    font-size: 13px;
    color: var(--t2);
    line-height: 20px;
  }
  .hint {
    font-size: 13px;
    color: var(--t2);
    font-weight: 600;
    text-align: right;
  }
  .lic {
    padding: 14px 0;
    align-items: flex-start;
  }
  .lic .hint {
    line-height: 20px;
    max-width: 70%;
  }
  .usage {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 0;
    box-shadow: inset 0 -1px 0 var(--line);
  }
  .big {
    font-family: var(--display);
    font-weight: 700;
    font-size: 22px;
    letter-spacing: -0.02em;
  }
  .ubar {
    display: flex;
    height: 8px;
    border-radius: 4px;
    background: var(--wave-e);
    overflow: hidden;
  }
  .ubar b {
    display: block;
    height: 100%;
  }
  .pin {
    background: var(--accent);
  }
  .oth {
    background: var(--wave-b);
  }
  .legend {
    gap: 18px;
    font-size: 12px;
    font-weight: 700;
    color: var(--t2);
  }
  .legend .row {
    gap: 6px;
  }
  .legend i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .danger-btn {
    border: none;
    background: none;
    padding: 0;
    color: var(--danger-text);
    font-size: 15px;
    font-weight: 700;
  }
  .danger-btn:hover {
    text-decoration: underline;
  }
  .pad .item {
    min-height: 58px;
  }
</style>
