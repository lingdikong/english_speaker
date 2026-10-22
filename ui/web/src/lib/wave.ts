// 确定性声波（UI_SPEC §3.1 / §3.2）：与设计稿 `seedOf / bars` 完全一致，三端共用。

/** FNV-1a 32 位（按 UTF-16 码元，与设计稿 `charCodeAt` 一致）。 */
export function seedOf(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h;
}

/** n 根竖条的高度（min..max，正弦包络 + LCG 随机）。 */
export function bars(seed: number, n: number, min: number, max: number): number[] {
  let x = seed || 1;
  const o: number[] = [];
  for (let i = 0; i < n; i++) {
    x = (Math.imul(x, 1103515245) + 12345) >>> 0;
    const r = (x >>> 8) / 16777216;
    const e = Math.sin((Math.PI * (i + 0.5)) / n);
    o.push(Math.round(min + (max - min) * Math.min(1, 0.3 * r + 0.7 * r * e + 0.15 * e)));
  }
  return o;
}

const memo = new Map<string, number[]>();

export function waveOf(key: string, n: number, min: number, max: number): number[] {
  const k = `${key}\u0000${n}\u0000${min}\u0000${max}`;
  let v = memo.get(k);
  if (!v) {
    v = bars(seedOf(key), n, min, max);
    memo.set(k, v);
  }
  return v;
}

/** 缩略图参数：边长 → 竖条数（72→11，56→9，48→7，38→6）。 */
export function thumbBars(size: number): number {
  if (size >= 72) return 11;
  if (size >= 56) return 9;
  if (size >= 48) return 7;
  return 6;
}
