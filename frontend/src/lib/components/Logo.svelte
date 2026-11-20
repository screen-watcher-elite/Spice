<script lang="ts">
  interface Props {
    size?: number;
    armed?: boolean;
  }
  let { size = 26, armed = true }: Props = $props();
</script>

<svg
  class="mark"
  class:breach={!armed}
  width={size}
  height={size}
  viewBox="0 0 32 32"
  fill="none"
  aria-hidden="true"
>
  <path class="hex" d="M16 3.5 27 9.75v12.5L16 28.5 5 22.25V9.75z" />
  <path class="mesh" d="M16 3.5V16M27 22.25 16 16M5 22.25 16 16" />
  <circle class="core" cx="16" cy="16" r="3.2" />
</svg>

<style>
  .mark {
    flex: none;
    overflow: visible;
  }
  .hex {
    stroke: var(--fg);
    stroke-width: 1.6;
    stroke-linejoin: round;
  }
  .mesh {
    stroke: var(--fg-3);
    stroke-width: 1.2;
    stroke-linecap: round;
  }
  .core {
    fill: var(--accent);
    filter: drop-shadow(0 0 4px rgba(var(--accent-rgb), 0.9));
    transition: fill 300ms var(--ease);
  }
  .breach .core {
    fill: var(--danger);
    filter: drop-shadow(0 0 4px rgba(var(--danger-rgb), 0.9));
    animation: blink 1.2s steps(2, jump-none) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
</style>
