// 跟随系统“减少动态效果”（UI_SPEC §1.1 第 8 条）：Svelte 过渡时长在开启时直接为 0。
const reduce = window.matchMedia('(prefers-reduced-motion: reduce)');

export function dur(ms: number): number {
  return reduce.matches ? 0 : ms;
}
