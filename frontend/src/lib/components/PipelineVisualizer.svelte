<script lang="ts">
  import type { LayerReport } from '../api';
  import { Shield, EyeOff, Brain, Scale, Lock, Radar } from 'lucide-svelte';

  interface Props {
    layers?: LayerReport[];
    shieldActive?: boolean;
    compact?: boolean;
  }
  let { layers = [], shieldActive = true, compact = false }: Props = $props();

  const defaults = [
    { layer: 1, name: 'Regex & Normalizer', sub: 'Unicode, Base64, Injection' },
    { layer: 2, name: 'DPDP PII Vault', sub: 'PAN, Aadhaar, Card, UPI' },
    { layer: 3, name: 'Gemini Decisions', sub: 'Zero-Token AI Guardrail' },
    { layer: 4, name: 'RBI Circuit Breaker', sub: 'Caps, Smurfing, IDOR, KYC' },
    { layer: 5, name: 'Outbound Firewall', sub: 'Egress Scrub & Leak Check' },
    { layer: 6, name: 'Async AI Sentinel', sub: 'Trajectory Watch & Revert' },
  ];

  function findReport(layerNum: number): LayerReport | undefined {
    return layers.find((l) => l.layer === layerNum);
  }

  function statusClass(rep?: LayerReport): string {
    if (!shieldActive) return 'bypassed';
    if (!rep) return 'idle';
    switch (rep.status) {
      case 'BLOCKED':
        return 'blocked';
      case 'HELD':
      case 'MASKED':
      case 'ALERT':
        return 'amber';
      case 'SKIPPED':
        return 'idle';
      default:
        return 'pass';
    }
  }

  function fmtUs(us?: number): string {
    if (us === undefined) return '—';
    if (us >= 1000) return `${(us / 1000).toFixed(2)}ms`;
    return `${us}µs`;
  }
</script>

<div class="pipeline-grid" class:compact>
  {#each defaults as item, idx}
    {@const rep = findReport(item.layer)}
    {@const st = statusClass(rep)}
    <div class="node {st}" style="animation-delay: {idx * 45}ms">
      <div class="node-top">
        <span class="layer-badge">L{item.layer}</span>
        <span class="icon-wrap">
          {#if item.layer === 1}<Shield size={14} />
          {:else if item.layer === 2}<EyeOff size={14} />
          {:else if item.layer === 3}<Brain size={14} />
          {:else if item.layer === 4}<Scale size={14} />
          {:else if item.layer === 5}<Lock size={14} />
          {:else}<Radar size={14} />{/if}
        </span>
        <span class="latency mono">{rep ? fmtUs(rep.latency_us) : 'ready'}</span>
      </div>
      <div class="node-title">{item.name}</div>
      <div class="node-sub">{rep ? rep.detail : item.sub}</div>
      {#if idx < defaults.length - 1}
        <div class="connector" aria-hidden="true">
          <div class="packet"></div>
        </div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .pipeline-grid {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 10px;
    position: relative;
  }
  .pipeline-grid.compact {
    gap: 8px;
  }
  .node {
    position: relative;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 11px 12px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    transition: transform 180ms var(--ease-out), border-color 180ms ease, background 180ms ease;
  }
  .node:hover {
    transform: translateY(-2px);
    border-color: var(--border-bright);
  }
  .node.pass {
    border-color: rgba(16, 185, 129, 0.35);
    background: linear-gradient(180deg, rgba(16, 185, 129, 0.06), var(--bg-2));
  }
  .node.amber {
    border-color: rgba(245, 158, 11, 0.42);
    background: linear-gradient(180deg, rgba(245, 158, 11, 0.08), var(--bg-2));
  }
  .node.blocked {
    border-color: rgba(244, 63, 94, 0.55);
    background: linear-gradient(180deg, rgba(244, 63, 94, 0.12), var(--bg-2));
  }
  .node.bypassed {
    border-color: rgba(244, 63, 94, 0.35);
    opacity: 0.65;
  }
  .node-top {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .layer-badge {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 4px;
    background: rgba(34, 211, 238, 0.12);
    color: var(--cyan);
  }
  .node.blocked .layer-badge {
    background: var(--crimson-dim);
    color: var(--crimson);
  }
  .node.amber .layer-badge {
    background: var(--amber-dim);
    color: var(--amber);
  }
  .icon-wrap {
    color: var(--text-1);
    display: inline-flex;
  }
  .latency {
    margin-left: auto;
    font-size: 10.5px;
    color: var(--text-1);
  }
  .node-title {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-0);
  }
  .node-sub {
    font-size: 11px;
    color: var(--text-1);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .connector {
    position: absolute;
    right: -11px;
    top: 50%;
    width: 11px;
    height: 2px;
    background: rgba(34, 211, 238, 0.22);
    overflow: hidden;
    z-index: 2;
  }
  .packet {
    width: 6px;
    height: 2px;
    background: var(--cyan);
    animation: packet-flow 2.2s linear infinite;
  }
  @media (max-width: 1180px) {
    .pipeline-grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    .connector {
      display: none;
    }
  }
</style>
