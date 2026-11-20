<script lang="ts">
  import type { LayerReport } from '../api';
  import { fmtUs, toneForLayer } from '../ui';
  import { ScanText, Fingerprint, Sparkles, Scale, ShieldCheck, Radar } from 'lucide-svelte';

  interface Props {
    layers?: LayerReport[];
    shieldActive?: boolean;
    compact?: boolean;
  }
  let { layers = [], shieldActive = true, compact = false }: Props = $props();

  const defaults = [
    { layer: 1, name: 'Normalizer', sub: 'Unicode · Base64 · injection regex' },
    { layer: 2, name: 'PII Vault', sub: 'PAN · Aadhaar · card · UPI tokenized' },
    { layer: 3, name: 'Gemini Decisions', sub: 'Zero-token semantic guardrail' },
    { layer: 4, name: 'Circuit Breaker', sub: 'RBI caps · smurfing · IDOR · KYC' },
    { layer: 5, name: 'Egress Firewall', sub: 'Outbound scrub & leak check' },
    { layer: 6, name: 'AI Sentinel', sub: 'Async trajectory watch & revert' },
  ];

  const byLayer = $derived(new Map(layers.map((l) => [l.layer, l])));
  const cascadeKey = $derived(layers.map((l) => `${l.layer}${l.status}${l.latency_us}`).join('|'));

  function stateOf(rep?: LayerReport): string {
    if (!shieldActive) return 'bypassed';
    if (!rep) return 'idle';
    return toneForLayer(rep.status);
  }
</script>

{#snippet icon(n: number, size: number)}
  {#if n === 1}<ScanText {size} strokeWidth={1.6} />
  {:else if n === 2}<Fingerprint {size} strokeWidth={1.6} />
  {:else if n === 3}<Sparkles {size} strokeWidth={1.6} />
  {:else if n === 4}<Scale {size} strokeWidth={1.6} />
  {:else if n === 5}<ShieldCheck {size} strokeWidth={1.6} />
  {:else}<Radar {size} strokeWidth={1.6} />{/if}
{/snippet}

{#key cascadeKey}
  {#if compact}
    <ol class="vlist">
      {#each defaults as item, idx}
        {@const rep = byLayer.get(item.layer)}
        {@const st = stateOf(rep)}
        <li class="vrow s-{st}" style="--i: {idx}">
          <div class="vrail">
            <span class="vorb">{@render icon(item.layer, 13)}</span>
          </div>
          <div class="vbody">
            <div class="vtop">
              <span class="mono lnum">L{item.layer}</span>
              <span class="vname">{item.name}</span>
              {#if rep}
                <span class="tag {st === 'mute' ? '' : st}">{rep.status}</span>
              {:else if !shieldActive}
                <span class="tag danger">OFF</span>
              {/if}
              <span class="mono vlat">{rep ? fmtUs(rep.latency_us) : '—'}</span>
            </div>
            <div class="vdetail">{rep ? rep.detail : item.sub}</div>
          </div>
        </li>
      {/each}
    </ol>
  {:else}
    <div class="mesh" class:off={!shieldActive}>
      <div class="track" aria-hidden="true">
        {#if shieldActive}<span class="beam"></span>{/if}
      </div>
      <ol class="nodes">
        {#each defaults as item, idx}
          {@const rep = byLayer.get(item.layer)}
          {@const st = stateOf(rep)}
          <li class="node s-{st}" style="--i: {idx}">
            <div class="orb">
              <span class="ring"></span>
              {@render icon(item.layer, 16)}
            </div>
            <div class="meta">
              <div class="row">
                <span class="mono lnum">L{item.layer}</span>
                <span class="mono lat">{rep ? fmtUs(rep.latency_us) : shieldActive ? 'ready' : 'off'}</span>
              </div>
              <div class="name">{item.name}</div>
              <div class="detail">{rep ? rep.detail : item.sub}</div>
            </div>
          </li>
        {/each}
      </ol>
    </div>
  {/if}
{/key}

<style>
  /* ---------- state colors ---------- */
  .s-ok { --c: var(--accent); --c-rgb: var(--accent-rgb); }
  .s-warn { --c: var(--warn); --c-rgb: var(--warn-rgb); }
  .s-danger { --c: var(--danger); --c-rgb: var(--danger-rgb); }
  .s-idle,
  .s-mute { --c: var(--fg-3); --c-rgb: 107, 107, 118; }
  .s-bypassed { --c: var(--danger); --c-rgb: var(--danger-rgb); }

  /* ---------- horizontal mesh ---------- */
  .mesh {
    position: relative;
  }
  .track {
    position: absolute;
    top: 21px;
    left: calc(100% / 12);
    right: calc(100% / 12);
    height: 1px;
    background: linear-gradient(90deg, transparent, var(--line-3) 6%, var(--line-3) 94%, transparent);
    overflow: hidden;
  }
  .mesh.off .track {
    background: repeating-linear-gradient(90deg, rgba(var(--danger-rgb), 0.5) 0 4px, transparent 4px 9px);
  }
  .beam {
    position: absolute;
    top: 0;
    left: 0;
    width: 18%;
    height: 1px;
    background: linear-gradient(90deg, transparent, var(--accent), transparent);
    box-shadow: 0 0 12px 1px rgba(var(--accent-rgb), 0.6);
    animation: travel 2.8s cubic-bezier(0.45, 0, 0.25, 1) infinite;
  }
  @keyframes travel {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(560%); }
  }

  .nodes {
    position: relative;
    list-style: none;
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 12px;
  }
  .node {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 14px;
    animation: rise 500ms var(--ease) both;
    animation-delay: calc(var(--i) * 70ms);
  }
  .orb {
    position: relative;
    width: 42px;
    height: 42px;
    border-radius: 13px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: radial-gradient(circle at 50% 30%, rgba(var(--c-rgb), 0.16), var(--orb-bg) 70%);
    border: 1px solid rgba(var(--c-rgb), 0.35);
    box-shadow: 0 0 0 4px var(--bg), 0 0 22px -4px rgba(var(--c-rgb), 0.45);
    transition: transform 260ms var(--ease-spring);
  }
  .node:hover .orb {
    transform: translateY(-2px) scale(1.04);
  }
  .s-idle .orb {
    background: var(--bg-2);
    border-color: var(--line-2);
    box-shadow: 0 0 0 4px var(--bg);
  }
  .s-bypassed .orb {
    background: var(--bg-1);
    border-style: dashed;
    opacity: 0.7;
    box-shadow: 0 0 0 4px var(--bg);
  }
  .ring {
    position: absolute;
    inset: -1px;
    border-radius: inherit;
    border: 1px solid var(--c);
    opacity: 0;
    animation: flash 900ms var(--ease) both;
    animation-delay: calc(var(--i) * 110ms + 150ms);
  }
  .s-idle .ring,
  .s-bypassed .ring {
    display: none;
  }
  @keyframes flash {
    0% { opacity: 0.9; transform: scale(1); }
    100% { opacity: 0; transform: scale(1.5); }
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    width: 100%;
  }
  .row {
    display: flex;
    justify-content: center;
    gap: 8px;
    font-size: 10.5px;
  }
  .lnum {
    color: var(--fg-3);
  }
  .lat {
    color: var(--c);
  }
  .s-idle .lat {
    color: var(--fg-3);
  }
  .name {
    font-size: 13px;
    font-weight: 500;
    color: var(--fg);
    letter-spacing: -0.01em;
  }
  .detail {
    font-size: 11.5px;
    color: var(--fg-3);
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    padding: 0 4px;
  }

  @media (max-width: 1080px) {
    .nodes {
      grid-template-columns: repeat(3, minmax(0, 1fr));
      row-gap: 26px;
    }
    .track {
      display: none;
    }
  }

  /* ---------- vertical (compact) ---------- */
  .vlist {
    list-style: none;
    display: flex;
    flex-direction: column;
  }
  .vrow {
    display: grid;
    grid-template-columns: 30px minmax(0, 1fr);
    gap: 12px;
    animation: rise 420ms var(--ease) both;
    animation-delay: calc(var(--i) * 60ms);
  }
  .vrail {
    position: relative;
    display: flex;
    justify-content: center;
  }
  .vrail::before {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 1px;
    background: var(--line-2);
  }
  .vrow:first-child .vrail::before {
    top: 14px;
  }
  .vrow:last-child .vrail::before {
    bottom: calc(100% - 14px);
  }
  .vorb {
    position: relative;
    z-index: 1;
    margin-top: 1px;
    width: 28px;
    height: 28px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: var(--bg-1);
    border: 1px solid rgba(var(--c-rgb), 0.35);
    box-shadow: 0 0 16px -4px rgba(var(--c-rgb), 0.5);
  }
  .s-idle .vorb {
    border-color: var(--line-2);
    box-shadow: none;
  }
  .s-bypassed .vorb {
    border-style: dashed;
  }
  .vbody {
    padding: 4px 0 16px;
    min-width: 0;
  }
  .vtop {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .vname {
    font-weight: 500;
    font-size: 13px;
  }
  .vlat {
    margin-left: auto;
    font-size: 11px;
    color: var(--c);
  }
  .s-idle .vlat {
    color: var(--fg-3);
  }
  .vdetail {
    font-size: 12px;
    color: var(--fg-3);
    margin-top: 2px;
    line-height: 1.45;
  }
</style>
