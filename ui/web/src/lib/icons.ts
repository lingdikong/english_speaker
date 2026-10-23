// 图标：线性图标 `P`（24 网格、1.75 描边）与播放条实心图标 `G`，路径照抄 docs/design/visual-v2.html。

export const P = {
  home: '<path d="M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z"/>',
  lib: '<path d="M5 4h4v16H5zM11 4h4v16h-4z"/><path d="m17 5 3.5.8-3 14.6-3.5-.8"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  prev: '<path d="M5 5v14"/><path d="M19 6v12l-9-6z"/>',
  next: '<path d="M19 5v14"/><path d="M5 6v12l9-6z"/>',
  loop: '<path d="M17 3l3 3-3 3"/><path d="M4 11V9a3 3 0 0 1 3-3h13"/><path d="M7 21l-3-3 3-3"/><path d="M20 13v2a3 3 0 0 1-3 3H4"/><path d="M11.5 10.5 13 9.5v5"/>',
  queue: '<path d="M4 6h11M4 12h11M4 18h7M18 14v6M15 17h6"/>',
  down: '<path d="m6 9 6 6 6-6"/>',
  close: '<path d="M6 6l12 12M18 6 6 18"/>',
  annot: '<path d="M4 8c2.5 4 6 4 8 0s5.5-4 8 0"/>',
  vol: '<path d="M11 5 6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7"/>',
  textsize: '<path d="M4 18 9 6l5 12M5.8 14h6.4"/><path d="M15 18l3-7 3 7M16 16h4"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>',
  wave: '<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/>',
  bookmark: '<path d="M6 3.5h12v17l-6-4-6 4z"/>',
  cloud: '<path d="M7 18a4.5 4.5 0 0 1-.5-9 6 6 0 0 1 11.5 1.5A3.8 3.8 0 0 1 17.5 18z"/><path d="M12 11v5M9.8 14l2.2 2 2.2-2"/>',
  moon: '<path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/>',
  // 以下为设计稿未画到、按同一风格补充的线性图标
  file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>',
  trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
  drag: '<path d="M8 7h8M8 12h8M8 17h8"/>',
} as const;

export type IconName = keyof typeof P;

export const G = {
  prev: '<svg width="25" height="25" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" aria-hidden="true"><path d="M11.5 6.8v10.4L4 12z"/><path d="M20 6.8v10.4L12.5 12z"/></svg>',
  next: '<svg width="25" height="25" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" aria-hidden="true"><path d="M4 6.8v10.4l7.5-5.2z"/><path d="M12.5 6.8v10.4l7.5-5.2z"/></svg>',
  play: '<svg width="28" height="28" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" aria-hidden="true"><path d="M7 4.8v14.4L19 12z"/></svg>',
  pause: '<svg width="26" height="26" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="5.5" y="4.5" width="4.5" height="15" rx="1.4"/><rect x="14" y="4.5" width="4.5" height="15" rx="1.4"/></svg>',
  loop: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M17 3.5l3 3-3 3"/><path d="M4 11.5V10a3.5 3.5 0 0 1 3.5-3.5H20"/><path d="M7 20.5l-3-3 3-3"/><path d="M20 12.5V14a3.5 3.5 0 0 1-3.5 3.5H4"/><path d="M11.3 10.6 12.8 9.6v4.8"/></svg>',
  lyrics: '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linejoin="round" aria-hidden="true"><path d="M4.5 4.5h15a1.5 1.5 0 0 1 1.5 1.5v9.5a1.5 1.5 0 0 1-1.5 1.5H12l-4.5 3.5V17h-3A1.5 1.5 0 0 1 3 15.5V6a1.5 1.5 0 0 1 1.5-1.5z"/><circle cx="9" cy="10" r="1.3" fill="currentColor" stroke="none"/><path d="M10.3 10.2c0 1.5-.7 2.5-1.9 2.9" stroke-width="1.6" stroke-linecap="round"/><circle cx="14.6" cy="10" r="1.3" fill="currentColor" stroke="none"/><path d="M15.9 10.2c0 1.5-.7 2.5-1.9 2.9" stroke-width="1.6" stroke-linecap="round"/></svg>',
  queue: '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><circle cx="4.5" cy="6.5" r="1.4" fill="currentColor" stroke="none"/><circle cx="4.5" cy="12" r="1.4" fill="currentColor" stroke="none"/><circle cx="4.5" cy="17.5" r="1.4" fill="currentColor" stroke="none"/><path d="M9 6.5h11M9 12h11M9 17.5h11"/></svg>',
  annot: '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" aria-hidden="true"><path d="M3.5 9.5c2.6 4.4 6 4.4 8.5 0s5.9-4.4 8.5 0"/><path d="M5 17h14" stroke-dasharray="0.5 3.5"/></svg>',
  vol: '<svg width="23" height="23" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" aria-hidden="true"><path d="M3.5 9.3h3.2l5-4.3v14l-5-4.3H3.5z" fill="currentColor" stroke-linejoin="round" stroke-width="1.2"/><path d="M15.3 9a4.2 4.2 0 0 1 0 6"/><path d="M18.3 6.3a8 8 0 0 1 0 11.4"/></svg>',
  mute: '<svg width="23" height="23" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" aria-hidden="true"><path d="M3.5 9.3h3.2l5-4.3v14l-5-4.3H3.5z" fill="currentColor" stroke-linejoin="round" stroke-width="1.2"/><path d="M15.5 9.5l5 5M20.5 9.5l-5 5"/></svg>',
};

export const MORE =
  '<svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="5" cy="12" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="19" cy="12" r="1.8"/></svg>';
export const PLAY = (s: number) =>
  `<svg width="${s}" height="${s}" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4.5v15l12-7.5z"/></svg>`;
export const PAUSE = (s: number) =>
  `<svg width="${s}" height="${s}" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4" height="14" rx="1.2"/><rect x="14" y="5" width="4" height="14" rx="1.2"/></svg>`;
export const SAVED = (s = 14) =>
  `<svg width="${s}" height="${s}" viewBox="0 0 24 24" fill="var(--accent)" aria-hidden="true"><path d="M6 3.5h12v17l-6-4-6 4z"/></svg>`;
