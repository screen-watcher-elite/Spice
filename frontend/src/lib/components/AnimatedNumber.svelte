<script lang="ts">
  interface Props {
    value: number;
    decimals?: number;
    suffix?: string;
    prefix?: string;
  }
  let { value, decimals = 0, suffix = '', prefix = '' }: Props = $props();

  let display = $state(0);
  let rafId = 0;

  $effect(() => {
    const target = Number.isFinite(value) ? value : 0;
    const start = display;
    const diff = target - start;
    if (Math.abs(diff) < 0.0001) {
      display = target;
      return;
    }
    const duration = 360;
    const t0 = performance.now();
    cancelAnimationFrame(rafId);

    const step = (now: number) => {
      const p = Math.min(1, (now - t0) / duration);
      const eased = 1 - Math.pow(1 - p, 3);
      display = start + diff * eased;
      if (p < 1) {
        rafId = requestAnimationFrame(step);
      } else {
        display = target;
      }
    };
    rafId = requestAnimationFrame(step);

    return () => cancelAnimationFrame(rafId);
  });

  const formatted = $derived(
    `${prefix}${decimals > 0 ? display.toFixed(decimals) : Math.round(display).toLocaleString('en-IN')}${suffix}`,
  );
</script>

<span class="tnum">{formatted}</span>
