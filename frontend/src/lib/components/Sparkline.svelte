<script lang="ts">
  interface Props {
    values: number[];
    color?: string;
    height?: number;
  }
  let { values, color = '#22d3ee', height = 38 }: Props = $props();

  const uid = Math.random().toString(36).slice(2, 8);
  const w = 180;

  const geometry = $derived.by(() => {
    const pts = values.length > 1 ? values : [0, 0];
    const min = Math.min(...pts);
    const max = Math.max(...pts);
    const span = max - min || 1;
    const padY = 5;
    const coords = pts.map((v, idx) => {
      const x = (idx / (pts.length - 1)) * w;
      const y = height - padY - ((v - min) / span) * (height - padY * 2);
      return { x, y };
    });
    const line = coords
      .map((p, i) => `${i === 0 ? 'M' : 'L'}${p.x.toFixed(1)},${p.y.toFixed(1)}`)
      .join(' ');
    const area = `${line} L${w},${height} L0,${height} Z`;
    const last = coords[coords.length - 1];
    return { line, area, last };
  });
</script>

<svg
  viewBox="0 0 {w} {height}"
  class="sparkline"
  style="width: 100%; height: {height}px; overflow: visible;"
  aria-hidden="true"
>
  <defs>
    <linearGradient id="sg-{uid}" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color={color} stop-opacity="0.28" />
      <stop offset="100%" stop-color={color} stop-opacity="0.0" />
    </linearGradient>
  </defs>
  <path d={geometry.area} fill="url(#sg-{uid})" />
  <path
    d={geometry.line}
    fill="none"
    stroke={color}
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
  <circle cx={geometry.last.x} cy={geometry.last.y} r="3" fill={color} />
</svg>
