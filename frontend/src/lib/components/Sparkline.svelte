<script lang="ts">
  interface Props {
    values: number[];
    color?: string;
    height?: number;
    grid?: boolean;
    dot?: boolean;
    fillOpacity?: number;
    strokeWidth?: number;
    baseline?: number | null;
  }
  let {
    values,
    color = 'var(--accent)',
    height = 38,
    grid = false,
    dot = true,
    fillOpacity = 0.22,
    strokeWidth = 1.5,
    baseline = null,
  }: Props = $props();

  const uid = Math.random().toString(36).slice(2, 8);
  const W = 400;

  const geo = $derived.by(() => {
    const raw = (values ?? []).filter((v) => Number.isFinite(v));
    const pts = raw.length > 1 ? raw : raw.length === 1 ? [raw[0], raw[0]] : [0, 0];
    let min = Math.min(...pts);
    let max = Math.max(...pts);
    if (baseline !== null) min = Math.min(min, baseline);
    const span = max - min || 1;
    const padTop = height * 0.14;
    const padBot = height * 0.08;
    const usable = height - padTop - padBot;
    const c = pts.map((v, i) => ({
      x: (i / (pts.length - 1)) * W,
      y: padTop + usable - ((v - min) / span) * usable,
    }));

    // Catmull-Rom → cubic Bézier for a calm, continuous curve
    let d = `M${c[0].x.toFixed(2)},${c[0].y.toFixed(2)}`;
    for (let i = 0; i < c.length - 1; i++) {
      const p0 = c[i - 1] ?? c[i];
      const p1 = c[i];
      const p2 = c[i + 1];
      const p3 = c[i + 2] ?? p2;
      const t = 0.18;
      const c1x = p1.x + (p2.x - p0.x) * t;
      const c1y = Math.min(height, Math.max(0, p1.y + (p2.y - p0.y) * t));
      const c2x = p2.x - (p3.x - p1.x) * t;
      const c2y = Math.min(height, Math.max(0, p2.y - (p3.y - p1.y) * t));
      d += ` C${c1x.toFixed(2)},${c1y.toFixed(2)} ${c2x.toFixed(2)},${c2y.toFixed(2)} ${p2.x.toFixed(2)},${p2.y.toFixed(2)}`;
    }
    const area = `${d} L${W},${height} L0,${height} Z`;
    const last = c[c.length - 1];
    return { d, area, lastX: (last.x / W) * 100, lastY: (last.y / height) * 100 };
  });
</script>

<div class="spark" style="height: {height}px; --c: {color};" aria-hidden="true">
  <svg viewBox="0 0 {W} {height}" preserveAspectRatio="none">
    <defs>
      <linearGradient id="sg-{uid}" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" stop-color={color} stop-opacity={fillOpacity} />
        <stop offset="100%" stop-color={color} stop-opacity="0" />
      </linearGradient>
      <linearGradient id="sl-{uid}" x1="0" y1="0" x2="1" y2="0">
        <stop offset="0%" stop-color={color} stop-opacity="0.15" />
        <stop offset="35%" stop-color={color} stop-opacity="0.9" />
        <stop offset="100%" stop-color={color} stop-opacity="1" />
      </linearGradient>
    </defs>
    {#if grid}
      {#each [0.25, 0.5, 0.75] as g}
        <line
          x1="0"
          x2={W}
          y1={height * g}
          y2={height * g}
          stroke="var(--grid-stroke)"
          stroke-dasharray="2 4"
          vector-effect="non-scaling-stroke"
        />
      {/each}
    {/if}
    <path d={geo.area} fill="url(#sg-{uid})" />
    <path
      d={geo.d}
      fill="none"
      stroke="url(#sl-{uid})"
      stroke-width={strokeWidth}
      stroke-linecap="round"
      stroke-linejoin="round"
      vector-effect="non-scaling-stroke"
    />
  </svg>
  {#if dot}
    <span class="tip" style="left: {geo.lastX}%; top: {geo.lastY}%;"></span>
  {/if}
</div>

<style>
  .spark {
    position: relative;
    width: 100%;
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  path {
    transition: d 600ms var(--ease);
  }
  .tip {
    position: absolute;
    width: 7px;
    height: 7px;
    margin: -3.5px 0 0 -3.5px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 18%, transparent), 0 0 14px var(--c);
    transition: top 600ms var(--ease);
  }
</style>
