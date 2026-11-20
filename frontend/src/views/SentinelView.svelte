<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { appState } from '../lib/stores/appState.svelte';
  import Sparkline from '../lib/components/Sparkline.svelte';
  import { fade, fly } from 'svelte/transition';
  import { Radar, Download, Check, ShieldAlert, RefreshCw } from 'lucide-svelte';

  let loading = $state(false);

  async function refresh() {
    loading = true;
    try {
      appState.sentinel = await api.getSentinelState();
    } finally {
      loading = false;
    }
  }

  async function ack(id: string) {
    try {
      await api.ackSentinelAlert(id);
      appState.pushToast('success', 'Alert Acknowledged', id);
      await refresh();
    } catch (e: unknown) {
      appState.pushToast('error', 'Failed', (e as Error).message);
    }
  }

  async function exportReport(fmt: 'json' | 'csv') {
    try {
      await api.downloadAuditReport(fmt);
      appState.pushToast('success', `Exported ${fmt.toUpperCase()} Audit Report`);
    } catch (e: unknown) {
      appState.pushToast('error', 'Admin Key Required for Export', (e as Error).message);
    }
  }

  onMount(refresh);
</script>

<div class="sentinel-page" in:fade={{ duration: 160 }}>
  <div class="panel top-banner">
    <div class="title-block">
      <div class="icon-circle"><Radar size={20} color="#22d3ee" /></div>
      <div>
        <h2>Layer 6 — Asynchronous AI Sentinel</h2>
        <p class="muted">
          Continuously audits multi-turn agent trajectories without blocking customer replies. Automatically quarantines sessions or reverts unauthorized payouts.
        </p>
      </div>
    </div>
    <div class="actions">
      <button type="button" class="btn" onclick={refresh} disabled={loading}>
        <RefreshCw size={14} /> Refresh
      </button>
      <button type="button" class="btn btn-primary" onclick={() => exportReport('json')}>
        <Download size={14} /> Export JSON
      </button>
      <button type="button" class="btn" onclick={() => exportReport('csv')}>
        <Download size={14} /> Export CSV
      </button>
    </div>
  </div>

  <div class="metrics-row">
    <div class="panel m-card">
      <span class="lbl">RBI Compliance Grade</span>
      <div class="big mono emerald">{appState.sentinel?.rbi_compliance_rating || 'AAA'}</div>
      <span class="sub">Score: {appState.sentinel?.alignment_drift_score?.toFixed(1) || '98.8'}%</span>
    </div>
    <div class="panel m-card">
      <span class="lbl">Total Turns Audited</span>
      <div class="big mono cyan">{appState.sentinel?.total_audited ?? 0}</div>
      <span class="sub">DPDP Minimization: {appState.sentinel?.dpdp_data_minimization_pct ?? 100}%</span>
    </div>
    <div class="panel m-card">
      <span class="lbl">Active Interventions</span>
      <div class="big mono amber">{appState.sentinel?.interventions_triggered ?? 0}</div>
      <span class="sub">Engine: {appState.sentinel?.last_engine_used || 'STANDBY'}</span>
    </div>
    <div class="panel m-card chart-card">
      <span class="lbl">Alignment Drift Trajectory</span>
      <Sparkline values={appState.sentinel?.recent_drift_trend || [98.5, 98.8]} color="#10b981" height={54} />
    </div>
  </div>

  <div class="panel audits-panel">
    <div class="audits-head">
      <h3>Autonomous Watcher Audit Trail</h3>
      <span class="chip chip-cyan">NON-BLOCKING TOKIO CHANNEL</span>
    </div>

    <div class="audit-list">
      {#if !appState.sentinel?.recent_audits?.length}
        <div class="empty">
          No asynchronous Sentinel audits yet. Trigger interactions in Sandbox or Customer Chats to watch Layer 6 evaluate multi-turn behavior.
        </div>
      {:else}
        {#each appState.sentinel.recent_audits as a (a.audit_id)}
          <div class="audit-card" in:fly={{ y: 8, duration: 150 }}>
            <div class="a-top">
              <div class="a-badges">
                <span class="mono id">{a.audit_id}</span>
                <span
                  class="chip {a.severity === 'CRITICAL' || a.severity === 'HIGH'
                    ? 'chip-crimson'
                    : a.severity === 'MEDIUM'
                      ? 'chip-amber'
                      : 'chip-emerald'}"
                >
                  {a.severity}
                </span>
                <span class="chip chip-muted">{a.engine}</span>
                {#if a.intervention !== 'NONE'}
                  <span class="chip chip-crimson">
                    <ShieldAlert size={11} /> {a.intervention}
                  </span>
                {/if}
              </div>
              <div class="a-right">
                <span class="mono muted">{a.drift_score.toFixed(1)}% ({a.rbi_grade})</span>
                {#if !a.acknowledged}
                  <button type="button" class="btn btn-sm btn-emerald" onclick={() => ack(a.audit_id)}>
                    <Check size={12} /> Acknowledge
                  </button>
                {:else}
                  <span class="chip chip-muted">Acked</span>
                {/if}
              </div>
            </div>
            <div class="a-summary">{a.summary}</div>
            <div class="a-meta mono muted">
              Session: {a.session_id || '—'} · Verdict: {a.verdict} · {new Date(a.created_at).toLocaleTimeString()}
            </div>
          </div>
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .sentinel-page {
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: 100%;
    overflow-y: auto;
  }
  .top-banner {
    padding: 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
  }
  .title-block {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .icon-circle {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: var(--cyan-dim);
    border: 1px solid rgba(34, 211, 238, 0.3);
    display: grid;
    place-items: center;
  }
  .title-block h2 {
    font-size: 15.5px;
    font-weight: 600;
  }
  .title-block p {
    font-size: 12.5px;
    max-width: 680px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .metrics-row {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }
  .m-card {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .lbl {
    font-size: 11.5px;
    color: var(--text-1);
  }
  .big {
    font-size: 26px;
    font-weight: 700;
  }
  .sub {
    font-size: 11.5px;
    color: var(--text-2);
  }
  .emerald { color: var(--emerald); }
  .cyan { color: var(--cyan); }
  .amber { color: var(--amber); }
  .muted { color: var(--text-2); }

  .audits-panel {
    flex: 1;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 280px;
  }
  .audits-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .audits-head h3 {
    font-size: 14px;
    font-weight: 600;
  }
  .audit-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow-y: auto;
  }
  .audit-card {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .a-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .a-badges {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
  }
  .a-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .id {
    font-size: 12px;
    font-weight: 600;
    color: var(--cyan);
  }
  .a-summary {
    font-size: 13px;
    color: var(--text-0);
  }
  .a-meta {
    font-size: 11px;
  }
  .empty {
    padding: 36px;
    text-align: center;
    color: var(--text-1);
  }
  @media (max-width: 1100px) {
    .metrics-row {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
