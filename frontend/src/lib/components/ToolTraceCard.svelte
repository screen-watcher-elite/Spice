<script lang="ts">
  import type { LayerReport, ToolTraceEntry } from '../api';
  import { slide } from 'svelte/transition';
  import { fmtUs, toneForLayer } from '../ui';
  import { ChevronRight, Check, ShieldAlert } from 'lucide-svelte';

  interface Props {
    layers?: LayerReport[];
    tools?: ToolTraceEntry[];
    decisionSource?: string;
    sentinelVerdict?: string;
    overheadMs?: number;
  }
  let { layers = [], tools = [], decisionSource, sentinelVerdict, overheadMs = 0 }: Props = $props();

  let expanded = $state(false);
  const heldCount = $derived(tools.filter((t) => !t.allowed).length);
</script>

{#if layers.length > 0 || tools.length > 0}
  <div class="trace" class:open={expanded}>
    <button type="button" class="head" onclick={() => (expanded = !expanded)} aria-expanded={expanded}>
      <span class="chev"><ChevronRight size={12} /></span>
      <span class="strip" aria-hidden="true">
        {#each layers as l}
          <i class="seg {toneForLayer(l.status)}" title="L{l.layer} {l.name} · {l.status}"></i>
        {/each}
      </span>
      <span class="lbl">trace</span>
      {#if tools.length > 0}
        <span class="meta">{tools.length} tool{tools.length > 1 ? 's' : ''}{#if heldCount > 0}<b class="t-warn"> · {heldCount} held</b>{/if}</span>
      {/if}
      {#if decisionSource}<span class="meta mono">{decisionSource}</span>{/if}
      {#if sentinelVerdict}
        <span class="meta mono {sentinelVerdict === 'BLOCKED' || sentinelVerdict === 'CIRCUIT_TRIP' ? 't-warn' : ''}">
          {sentinelVerdict}
        </span>
      {/if}
      <span class="ms mono">{overheadMs.toFixed(2)}ms</span>
    </button>

    {#if expanded}
      <div class="body" transition:slide={{ duration: 220 }}>
        {#if tools.length > 0}
          <div class="sec">
            <div class="eyebrow">Tool calls</div>
            {#each tools as tc}
              <div class="tool" class:held={!tc.allowed}>
                <div class="tool-top">
                  <span class="ico">
                    {#if tc.allowed}<Check size={12} strokeWidth={2.4} />{:else}<ShieldAlert size={12} />{/if}
                  </span>
                  <span class="mono fn">{tc.name}<span class="muted">()</span></span>
                  <span class="tag {tc.allowed ? 'ok' : 'warn'}">
                    {tc.allowed ? (tc.executed ? 'executed' : 'allowed') : tc.violation_code || 'held'}
                  </span>
                </div>
                <pre class="code">{JSON.stringify(tc.args, null, 2)}</pre>
                <div class="reason">{tc.reason}</div>
                {#if tc.output}
                  <details>
                    <summary>Ledger result</summary>
                    <pre class="code">{JSON.stringify(tc.output, null, 2)}</pre>
                  </details>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        {#if layers.length > 0}
          <div class="sec">
            <div class="eyebrow">Gateway inspection</div>
            <div class="layers">
              {#each layers as l}
                <div class="lrow">
                  <span class="mono muted">L{l.layer}</span>
                  <span class="lname">{l.name}</span>
                  <span class="mono muted lat">{fmtUs(l.latency_us)}</span>
                  <span class="tag {toneForLayer(l.status)}">{l.status}</span>
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
  .trace {
    margin-top: 6px;
    border-radius: 10px;
    border: 1px solid transparent;
    transition: border-color 200ms var(--ease), background 200ms var(--ease);
  }
  .trace.open {
    border-color: var(--line);
    background: var(--well);
  }
  .head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 5px 8px;
    background: transparent;
    border: none;
    border-radius: 10px;
    cursor: pointer;
    color: var(--fg-3);
    font-size: 11px;
    text-align: left;
    transition: color 160ms, background 160ms;
  }
  .head:hover {
    color: var(--fg-2);
    background: var(--s-1);
  }
  .chev {
    display: inline-flex;
    transition: transform 220ms var(--ease);
  }
  .open .chev {
    transform: rotate(90deg);
  }
  .strip {
    display: inline-flex;
    gap: 2px;
  }
  .seg {
    width: 9px;
    height: 4px;
    border-radius: 2px;
    background: var(--accent);
    opacity: 0.8;
  }
  .seg.warn { background: var(--warn); }
  .seg.danger { background: var(--danger); }
  .seg.mute { background: var(--fg-4); }
  .lbl {
    font-family: var(--font-mono);
    text-transform: uppercase;
    letter-spacing: 0.12em;
    font-size: 9.5px;
  }
  .meta {
    font-size: 11px;
  }
  .meta::before {
    content: '·';
    margin-right: 8px;
    color: var(--fg-4);
  }
  .ms {
    margin-left: auto;
    color: var(--fg-2);
  }
  .body {
    padding: 4px 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .sec {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .tool {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--s-1);
    border: 1px solid var(--line);
  }
  .tool.held {
    border-color: rgba(var(--warn-rgb), 0.25);
    background: rgba(var(--warn-rgb), 0.03);
  }
  .tool-top {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ico {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 6px;
    background: rgba(var(--accent-rgb), 0.1);
    color: var(--accent);
  }
  .held .ico {
    background: rgba(var(--warn-rgb), 0.1);
    color: var(--warn);
  }
  .fn {
    font-size: 12px;
    color: var(--fg);
    flex: 1;
  }
  .code {
    font-size: 11px;
    padding: 8px 10px;
    max-height: 160px;
    overflow: auto;
  }
  .reason {
    font-size: 12px;
    color: var(--fg-2);
  }
  details summary {
    font-size: 11px;
    color: var(--fg-3);
    cursor: pointer;
    user-select: none;
    margin-bottom: 6px;
  }
  details summary:hover {
    color: var(--fg);
  }
  .layers {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 4px 14px;
  }
  .lrow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 0;
    font-size: 11.5px;
    border-bottom: 1px dashed var(--line);
  }
  .lname {
    color: var(--fg-2);
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lat {
    font-size: 10.5px;
  }
</style>
