<script lang="ts">
  import type { LayerReport, ToolTraceEntry } from '../api';
  import { slide } from 'svelte/transition';
  import { ChevronDown, ChevronRight, Cpu, ShieldAlert, CheckCircle2, Clock } from 'lucide-svelte';

  interface Props {
    layers?: LayerReport[];
    tools?: ToolTraceEntry[];
    decisionSource?: string;
    sentinelVerdict?: string;
    overheadMs?: number;
  }
  let { layers = [], tools = [], decisionSource, sentinelVerdict, overheadMs = 0 }: Props = $props();

  let expanded = $state(false);
</script>

{#if (layers && layers.length > 0) || (tools && tools.length > 0)}
  <div class="trace-box">
    <button type="button" class="trace-toggle" onclick={() => (expanded = !expanded)}>
      <span class="left">
        {#if expanded}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
        <Cpu size={13} />
        <span>Model & Security Trace</span>
        {#if tools && tools.length > 0}
          <span class="chip chip-cyan">{tools.length} tool{tools.length > 1 ? 's' : ''}</span>
        {/if}
        {#if decisionSource}
          <span class="chip chip-muted">{decisionSource}</span>
        {/if}
        {#if sentinelVerdict}
          <span
            class="chip {sentinelVerdict === 'BLOCKED' || sentinelVerdict === 'CIRCUIT_TRIP'
              ? 'chip-amber'
              : 'chip-emerald'}"
          >
            {sentinelVerdict}
          </span>
        {/if}
      </span>
      <span class="right mono"><Clock size={11} /> {overheadMs.toFixed(2)} ms</span>
    </button>

    {#if expanded}
      <div class="trace-body" transition:slide={{ duration: 180 }}>
        {#if tools && tools.length > 0}
          <div class="section">
            <div class="sec-label">Model Tool Execution</div>
            {#each tools as tc}
              <div class="tool-item" class:blocked={!tc.allowed}>
                <div class="tool-head">
                  <span class="mono tool-name">
                    {#if tc.allowed}
                      <CheckCircle2 size={13} color="#10b981" />
                    {:else}
                      <ShieldAlert size={13} color="#f59e0b" />
                    {/if}
                    {tc.name}()
                  </span>
                  <span class="chip {tc.allowed ? 'chip-emerald' : 'chip-amber'}">
                    {tc.allowed ? (tc.executed ? 'EXECUTED' : 'ALLOWED') : tc.violation_code || 'HELD'}
                  </span>
                </div>
                <pre class="mono code-block">{JSON.stringify(tc.args, null, 2)}</pre>
                <div class="tool-reason">{tc.reason}</div>
                {#if tc.output}
                  <details class="out-details">
                    <summary>Database Tool Result</summary>
                    <pre class="mono code-block">{JSON.stringify(tc.output, null, 2)}</pre>
                  </details>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        {#if layers && layers.length > 0}
          <div class="section">
            <div class="sec-label">6-Layer Gateway Inspection</div>
            <div class="layer-pills">
              {#each layers as l}
                <div class="l-pill">
                  <span class="mono">L{l.layer}</span>
                  <span>{l.name}</span>
                  <span class="mono muted">{l.latency_us}µs</span>
                  <span
                    class="chip {l.status === 'BLOCKED'
                      ? 'chip-crimson'
                      : l.status === 'HELD' || l.status === 'MASKED'
                        ? 'chip-amber'
                        : 'chip-emerald'}"
                  >
                    {l.status}
                  </span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .trace-box {
    margin-top: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: rgba(6, 10, 17, 0.72);
    overflow: hidden;
  }
  .trace-toggle {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 10px;
    background: transparent;
    border: none;
    color: var(--text-1);
    font-size: 11.5px;
    font-family: inherit;
    cursor: pointer;
    transition: background 140ms ease, color 140ms ease;
  }
  .trace-toggle:hover {
    background: rgba(148, 163, 184, 0.06);
    color: var(--text-0);
  }
  .left {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .right {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--cyan);
  }
  .trace-body {
    padding: 10px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .sec-label {
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-2);
    margin-bottom: 6px;
  }
  .tool-item {
    border: 1px solid var(--border);
    background: var(--bg-1);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 6px;
  }
  .tool-item.blocked {
    border-color: rgba(245, 158, 11, 0.35);
  }
  .tool-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .tool-name {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-0);
  }
  .code-block {
    font-size: 11px;
    background: var(--bg-0);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 6px 8px;
    color: #cbd5e1;
    overflow-x: auto;
    max-height: 140px;
  }
  .tool-reason {
    font-size: 11.5px;
    color: var(--text-1);
  }
  .out-details summary {
    font-size: 11px;
    color: var(--cyan);
    cursor: pointer;
    user-select: none;
  }
  .layer-pills {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 6px;
  }
  .l-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border-radius: 4px;
    background: var(--bg-1);
    border: 1px solid var(--border);
    font-size: 11px;
  }
  .muted {
    margin-left: auto;
    color: var(--text-2);
  }
</style>
