<script lang="ts">
  // 线性图标（24 网格、1.75 描边，路径见 lib/icons.ts 的 P）。
  import { P, type IconName } from '../lib/icons';

  let { name, size = 22, stroke = 1.75 }: { name: IconName; size?: number; stroke?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true">{@html P[name]}</svg>
