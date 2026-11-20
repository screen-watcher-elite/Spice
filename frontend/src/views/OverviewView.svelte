<script lang="ts">
  import { appState } from '../lib/stores/appState.svelte';
  import type { TelemetryEvent } from '../lib/api';
  import AnimatedNumber from '../lib/components/AnimatedNumber.svelte';
  import Sparkline from '../lib/components/Sparkline.svelte';
  import PipelineVisualizer from '../lib/components/PipelineVisualizer.svelte';
  import { fly, fade } from 'svelte/transition';
  import { ShieldAlert, EyeOff, Activity, UserCheck, Radar, X, ArrowUpRight } from 'lucide-svelte';

  let filterType = $state<string>('ALL');
  let selectedEvent = $state<TelemetryEvent | null>(null);

  const filters = ['ALL', 'BLOCKED', 'MASKED', 'CIRCUIT_TRIP', 'STAFF_OVERRIDE', 'SENTINEL_ALERT'];

  const filteredEvents = $derived(
    filterType === 'ALL'
      ? appState.events
      : appState.events.filter((e) => e.event_type === filterType),
  );

  function chipForEvent(t: string): string {
    switch (t) {
      case 'BLOCKED':
      case 'UNPROTECTED_RUN':
      case 'SENTINEL_ALERT':
        return 'chip-crimson';
      case 'CIRCUIT_TRIP':
      case 'MASKED':
        return 'chip-amber';
      case 'STAFF_OVERRIDE':
        return 'chip-violet';
      default:
        return 'chip-emerald';
    }
  }
</script>

<div class="overview" in:fade={{ duration: 160 }}>
  {#if !appState.quickStartDismissed}
    <div class="panel quickstart">
      <div class="qs-text">
        <span class="chip chip-cyan">QUICK START</span>
        <span>New here? Test live attack protection, approve a held ₹25,000 refund, or connect Gemini in 1 click.</span>
      </div>
      <div class="qs-btns">
        <button type="button" class="btn btn-sm btn-primary" onclick={() => (appState.activeTab = 'sandbox')}>
          Test an Attack
        </button>
        <button type="button" class="btn btn-sm btn-amber" onclick={() => (appState.activeTab = 'chats')}>
          Review Held Refunds ({appState.pendingOverridesCount})
        </button>
        <button type="button" class="btn btn-sm" onclick={() => (appState.activeTab = 'settings')}>
          API & Policy
        </button>
        <button
          type="button"
          class="btn btn-sm"
          aria-label="Dismiss quick start"
          onclick={() => (appState.quickStartDismissed = true)}
        >
          <X size={13} />
        </button>
      </div>
    </div>
  {/if}

  <!-- KPI Grid -->
  <div class="kpi-grid">
    <div class="panel kpi-card">
      <div class="kpi-head">
        <span class="kpi-label">Inline Latency (Avg)</span>
        <Activity size={15} color="#22d3ee" />
      </div>
      <div class="kpi-val cyan">
        <AnimatedNumber value={appState.stats.avg_overhead_ms} decimals={2} suffix=" ms" />
      </div>
      <div class="kpi-sub mono">
        P50 {appState.stats.p50_overhead_ms.toFixed(2)}ms · P99 {appState.stats.p99_overhead_ms.toFixed(2)}ms
      </div>
      <Sparkline values={appState.stats.recent_latencies_ms} color="#22d3ee" height={34} />
    </div>

    <div class="panel kpi-card">
      <div class="kpi-head">
        <span class="kpi-label">Threats Blocked</span>
        <ShieldAlert size={15} color="#f43f5e" />
      </div>
      <div class="kpi-val crimson">
        <AnimatedNumber value={appState.stats.blocked_attacks} />
      </div>
      <div class="kpi-sub">Prompt injections & overrides stopped</div>
      <Sparkline values={[0, 1, 1, 2, appState.stats.blocked_attacks]} color="#f43f5e" height={34} />
    </div>

    <div class="panel kpi-card">
      <div class="kpi-head">
        <span class="kpi-label">PII Tokens Masked</span>
        <EyeOff size={15} color="#f59e0b" />
      </div>
      <div class="kpi-val amber">
        <AnimatedNumber value={appState.stats.pii_masked_count} />
      </div>
      <div class="kpi-sub">DPDP Act 2023 vault isolations</div>
      <Sparkline values={[0, 1, 2, appState.stats.pii_masked_count]} color="#f59e0b" height={34} />
    </div>

    <button type="button" class="panel kpi-card clickable" onclick={() => (appState.activeTab = 'chats')}>
      <div class="kpi-head">
        <span class="kpi-label">Pending Overrides</span>
        <UserCheck size={15} color="#a78bfa" />
      </div>
      <div class="kpi-val violet">
        <AnimatedNumber value={appState.pendingOverridesCount} />
      </div>
      <div class="kpi-sub action-link">
        <span>Open Maker-Checker queue</span>
        <ArrowUpRight size={13} />
      </div>
    </button>

    <button type="button" class="panel kpi-card clickable" onclick={() => (appState.activeTab = 'sentinel')}>
      <div class="kpi-head">
        <span class="kpi-label">RBI Compliance Grade</span>
        <Radar size={15} color="#10b981" />
      </div>
      <div class="kpi-val emerald mono">
        {appState.sentinel?.rbi_compliance_rating || 'AAA'}
      </div>
      <div class="kpi-sub">
        Drift score {appState.sentinel?.alignment_drift_score?.toFixed(1) || '98.8'}% · {appState.sentinel?.total_audited || 0} audited
      </div>
      <Sparkline values={appState.sentinel?.recent_drift_trend || [98.5, 98.8]} color="#10b981" height={34} />
    </button>
  </div>

  <!-- 6-Layer Security Pipeline Visualizer -->
  <div class="panel section-box">
    <div class="section-head">
      <div>
        <h2>6-Layer Inline & Async Security Mesh</h2>
        <p class="muted">Real-time execution latency and status from the latest inspected transaction.</p>
      </div>
      <span class="chip {appState.shieldStatus?.shield_active ? 'chip-cyan' : 'chip-crimson'}">
        {appState.shieldStatus?.shield_active ? '6 LAYERS ENFORCED' : 'PERIMETER BYPASSED'}
      </span>
    </div>
    <PipelineVisualizer
      layers={appState.lastLayers}
      shieldActive={appState.shieldStatus?.shield_active ?? true}
    />
  </div>

  <!-- Real-Time Telemetry Feed -->
  <div class="panel feed-box">
    <div class="section-head">
      <div>
        <h2>Live Telemetry Stream</h2>
        <p class="muted">Zero plaintext PII — customer secrets stay strictly in local ephemeral vault.</p>
      </div>
      <div class="filter-row">
        {#each filters as f}
          <button
            type="button"
            class="filter-chip"
            class:active={filterType === f}
            onclick={() => (filterType = f)}
          >
            {f.replace('_', ' ')}
          </button>
        {/each}
      </div>
    </div>

    <div class="table-wrap">
      {#if filteredEvents.length === 0}
        <div class="empty">
          No telemetry events matching filter yet. Run a scenario in <b>Attack Sandbox</b> or message a customer in <b>Customer Chats</b>.
        </div>
      {:else}
        <table>
          <thead>
            <tr>
              <th>Time</th>
              <th>Event</th>
              <th>Account</th>
              <th>Sanitized Payload</th>
              <th>Engine / Source</th>
              <th class="num-col">Latency</th>
            </tr>
          </thead>
          <tbody>
            {#each filteredEvents as ev (ev.event_id)}
              <tr class="ev-row" onclick={() => (selectedEvent = ev)}>
                <td class="mono muted">{ev.iso_time}</td>
                <td><span class="chip {chipForEvent(ev.event_type)}">{ev.event_type}</span></td>
                <td class="mono">{ev.account_id || '—'}</td>
                <td class="prompt-cell">{ev.user_prompt_sanitized}</td>
                <td class="mono muted">{ev.decision_source || ev.target_model}</td>
                <td class="mono num-col cyan">{ev.overhead_ms.toFixed(2)}ms</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>
</div>

<!-- Forensic Slide-Over Drawer -->
{#if selectedEvent}
  <div class="drawer-backdrop" transition:fade={{ duration: 150 }}>
    <button
      type="button"
      class="backdrop-dismiss"
      aria-label="Close inspector"
      onclick={() => (selectedEvent = null)}
    ></button>
    <div
      class="drawer panel"
      role="dialog"
      aria-modal="true"
      aria-label="Forensic event inspector"
      transition:fly={{ x: 320, duration: 220 }}
    >
      <div class="drawer-head">
        <div>
          <span class="chip {chipForEvent(selectedEvent.event_type)}">{selectedEvent.event_type}</span>
          <h3 class="mono">{selectedEvent.event_id}</h3>
        </div>
        <button type="button" class="btn btn-sm" onclick={() => (selectedEvent = null)}>
          <X size={15} />
        </button>
      </div>

      <div class="drawer-body">
        <div class="meta-grid">
          <div><span class="lbl">Time</span><div class="mono">{selectedEvent.iso_time}</div></div>
          <div><span class="lbl">Account</span><div class="mono">{selectedEvent.account_id || '—'}</div></div>
          <div><span class="lbl">Overhead</span><div class="mono cyan">{selectedEvent.overhead_ms.toFixed(3)} ms</div></div>
          <div><span class="lbl">Source</span><div class="mono">{selectedEvent.decision_source || '—'}</div></div>
        </div>

        {#if selectedEvent.reason}
          <div class="alert-box">
            <strong>Policy Decision:</strong> {selectedEvent.reason}
          </div>
        {/if}

        <div class="sub-block">
          <div class="lbl">Sanitized Prompt (Seen by LLM)</div>
          <pre class="mono code">{selectedEvent.user_prompt_sanitized}</pre>
        </div>

        {#if selectedEvent.layers && selectedEvent.layers.length > 0}
          <div class="sub-block">
            <div class="lbl">Layer Timing Breakdown</div>
            <div class="layer-list">
              {#each selectedEvent.layers as l}
                <div class="layer-item">
                  <span class="mono">L{l.layer}</span>
                  <span class="l-name">{l.name}</span>
                  <span class="mono muted">{l.latency_us}µs</span>
                  <span class="chip chip-muted">{l.status}</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if selectedEvent.tool_name}
          <div class="sub-block">
            <div class="lbl">Tool Evaluated: <span class="mono">{selectedEvent.tool_name}</span></div>
            <pre class="mono code">{JSON.stringify(selectedEvent.tool_args, null, 2)}</pre>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    overflow-y: auto;
    padding-right: 4px;
  }
  .quickstart {
    padding: 10px 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    background: linear-gradient(90deg, rgba(34, 211, 238, 0.08), var(--bg-1));
    border-color: rgba(34, 211, 238, 0.25);
  }
  .qs-text {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12.5px;
    color: var(--text-0);
  }
  .qs-btns {
    display: flex;
    gap: 7px;
    flex-wrap: wrap;
  }
  .kpi-grid {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 12px;
  }
  .kpi-card {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .kpi-card.clickable {
    cursor: pointer;
    text-align: left;
    font: inherit;
    color: inherit;
  }
  .kpi-card.clickable:hover {
    border-color: var(--border-bright);
    transform: translateY(-2px);
  }
  .kpi-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .kpi-label {
    font-size: 11.5px;
    font-weight: 500;
    color: var(--text-1);
  }
  .kpi-val {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.02em;
    margin-top: 2px;
  }
  .cyan { color: var(--cyan); }
  .crimson { color: var(--crimson); }
  .amber { color: var(--amber); }
  .emerald { color: var(--emerald); }
  .violet { color: var(--violet); }
  .muted { color: var(--text-2); }

  .kpi-sub {
    font-size: 11px;
    color: var(--text-2);
    margin-bottom: 4px;
  }
  .action-link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--violet);
    margin-top: auto;
  }

  .section-box,
  .feed-box {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .feed-box {
    flex: 1;
    min-height: 270px;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .section-head h2 {
    font-size: 14.5px;
    font-weight: 600;
  }
  .section-head p {
    font-size: 12px;
  }
  .filter-row {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .filter-chip {
    padding: 4px 9px;
    font-size: 11px;
    font-weight: 600;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--bg-2);
    color: var(--text-1);
    cursor: pointer;
    transition: all 140ms ease;
  }
  .filter-chip.active {
    background: var(--cyan-dim);
    border-color: rgba(34, 211, 238, 0.45);
    color: var(--cyan);
  }
  .table-wrap {
    overflow: auto;
    flex: 1;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  th {
    text-align: left;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-2);
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    background: var(--bg-1);
  }
  td {
    padding: 9px 10px;
    border-bottom: 1px solid rgba(148, 163, 184, 0.06);
  }
  .ev-row {
    cursor: pointer;
    transition: background 120ms ease;
  }
  .ev-row:hover {
    background: rgba(34, 211, 238, 0.05);
  }
  .prompt-cell {
    max-width: 420px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .num-col {
    text-align: right;
  }
  .empty {
    padding: 36px;
    text-align: center;
    color: var(--text-1);
  }

  .drawer-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(4, 7, 12, 0.65);
    backdrop-filter: blur(4px);
    z-index: 40;
    display: flex;
    justify-content: flex-end;
  }
  .backdrop-dismiss {
    flex: 1;
    border: none;
    background: transparent;
    cursor: pointer;
  }
  .drawer {
    position: relative;
    z-index: 1;
    width: min(480px, 92vw);
    height: 100%;
    border-radius: 0;
    border-left: 1px solid var(--border-bright);
    display: flex;
    flex-direction: column;
    padding: 18px;
    gap: 14px;
    overflow-y: auto;
  }
  .drawer-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }
  .drawer-head h3 {
    margin-top: 6px;
    font-size: 15px;
  }
  .drawer-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .meta-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 10px;
    background: var(--bg-2);
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
  }
  .lbl {
    font-size: 11px;
    text-transform: uppercase;
    color: var(--text-2);
    margin-bottom: 3px;
  }
  .alert-box {
    background: var(--amber-dim);
    border: 1px solid rgba(245, 158, 11, 0.3);
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    font-size: 12.5px;
  }
  .code {
    background: var(--bg-0);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 10px;
    font-size: 11.5px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .layer-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .layer-item {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-2);
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    font-size: 12px;
  }
  .l-name {
    flex: 1;
  }
  @media (max-width: 1200px) {
    .kpi-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
