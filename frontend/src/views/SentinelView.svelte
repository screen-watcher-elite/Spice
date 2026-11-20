<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import { appState } from '../lib/stores/appState.svelte';
  import AnimatedNumber from '../lib/components/AnimatedNumber.svelte';
  import Sparkline from '../lib/components/Sparkline.svelte';
  import { fly } from 'svelte/transition';
  import { toneForSeverity, fmtClock, humanize } from '../lib/ui';
  import {
    Radar,
    Download,
    Check,
    ShieldAlert,
    RefreshCw,
    ArrowUpRight,
  } from 'lucide-svelte';

  let loading = $state(false);
  let auditFilter = $state<'ALL' | 'INTERVENTIONS' | 'UNACKED'>('ALL');

  const audits = $derived.by(() => {
    const list = appState.sentinel?.recent_audits ?? [];
    if (auditFilter === 'INTERVENTIONS') return list.filter((a) => a.intervention !== 'NONE');
    if (auditFilter === 'UNACKED') return list.filter((a) => !a.acknowledged);
    return list;
  });

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
      appState.pushToast('success', 'Alert acknowledged', id);
      await refresh();
    } catch (e: unknown) {
      appState.pushToast('error', 'Failed to acknowledge', (e as Error).message);
    }
  }

  async function exportReport(fmt: 'json' | 'csv') {
    try {
      await api.downloadAuditReport(fmt);
      appState.pushToast('success', `Exported ${fmt.toUpperCase()} compliance report`);
    } catch (e: unknown) {
      const msg = (e as Error).message;
      if (msg.includes('Unauthorized') || msg.includes('Admin Key') || msg.includes('401')) {
        appState.requestAdminUnlock(msg);
      } else {
        appState.pushToast('error', 'Export failed', msg);
      }
    }
  }

  onMount(refresh);
</script>

<div class="sentinel-page">
  <!-- Header Card -->
  <section class="card hero-banner">
    <div class="hb-left">
      <div class="radar-orb">
        <Radar size={20} strokeWidth={1.7} />
      </div>
      <div>
        <div class="eyebrow">Layer 6 · Out-of-Band Watcher</div>
        <h2>Asynchronous AI Sentinel & Compliance Ledger</h2>
        <p class="muted">
          Continuously audits multi-turn agent trajectories over a non-blocking Tokio channel — quarantining sessions or reverting unauthorized payouts with zero inline latency.
        </p>
      </div>
    </div>

    <div class="hb-actions">
      <button type="button" class="btn btn-sm" onclick={refresh} disabled={loading}>
        <RefreshCw size={13} />
        <span>{loading ? 'Syncing…' : 'Refresh'}</span>
      </button>
      <button type="button" class="btn btn-sm" onclick={() => exportReport('csv')}>
        <Download size={13} /> Export CSV
      </button>
      <button type="button" class="btn btn-sm btn-solid" onclick={() => exportReport('json')}>
        <Download size={13} /> Export JSON
      </button>
    </div>
  </section>

  <!-- 4-Card Metrics Bento -->
  <section class="metrics-row" aria-label="Sentinel compliance metrics">
    <div class="card m-card grade-card">
      <div class="mc-top">
        <span class="eyebrow">RBI Compliance Grade</span>
        <span class="tag ok dotted">Aligned</span>
      </div>
      <div class="big mono t-accent">
        {appState.sentinel?.rbi_compliance_rating || 'AAA'}
      </div>
      <div class="score-bar">
        <div
          class="score-fill"
          style="width: {Math.min(100, Math.max(0, appState.sentinel?.alignment_drift_score ?? 98.8))}%"
        ></div>
      </div>
      <div class="sub mono">
        Alignment score {(appState.sentinel?.alignment_drift_score ?? 98.8).toFixed(1)}%
      </div>
    </div>

    <div class="card m-card">
      <div class="mc-top">
        <span class="eyebrow">Turns Audited</span>
        <span class="tag plain mono">Async</span>
      </div>
      <div class="big mono">
        <AnimatedNumber value={appState.sentinel?.total_audited ?? 0} />
      </div>
      <div class="sub">
        DPDP data minimization <b class="mono t-accent">{appState.sentinel?.dpdp_data_minimization_pct ?? 100}%</b>
      </div>
    </div>

    <div class="card m-card">
      <div class="mc-top">
        <span class="eyebrow">Interventions</span>
        <span class="tag {(appState.sentinel?.interventions_triggered ?? 0) > 0 ? 'warn' : 'ok'}">
          {appState.sentinel?.active_alerts ?? 0} active
        </span>
      </div>
      <div class="big mono t-warn">
        <AnimatedNumber value={appState.sentinel?.interventions_triggered ?? 0} />
      </div>
      <div class="sub mono">
        Engine: {appState.sentinel?.last_engine_used || 'STANDBY'}
      </div>
    </div>

    <div class="card m-card chart-card">
      <div class="mc-top">
        <span class="eyebrow">Alignment Drift Trajectory</span>
        <span class="mono muted" style="font-size: 11px;">Multi-turn stability</span>
      </div>
      <div class="chart-wrap">
        <Sparkline
          values={appState.sentinel?.recent_drift_trend?.length
            ? appState.sentinel.recent_drift_trend
            : [98.4, 98.7, 98.8, 99.1, 98.9]}
          color="var(--accent)"
          height={58}
          grid={true}
        />
      </div>
    </div>
  </section>

  <!-- Audit Trail -->
  <section class="card audits-panel">
    <header class="audits-head">
      <div class="ah-left">
        <h3>Autonomous Watcher Audit Trail</h3>
        <span class="tag ok dotted">Non-Blocking Tokio Channel</span>
      </div>

      <div class="filter-pills" role="tablist">
        <button
          type="button"
          role="tab"
          class="f-pill"
          class:active={auditFilter === 'ALL'}
          onclick={() => (auditFilter = 'ALL')}
        >
          All Audits ({appState.sentinel?.recent_audits?.length ?? 0})
        </button>
        <button
          type="button"
          role="tab"
          class="f-pill"
          class:active={auditFilter === 'INTERVENTIONS'}
          onclick={() => (auditFilter = 'INTERVENTIONS')}
        >
          Interventions
        </button>
        <button
          type="button"
          role="tab"
          class="f-pill"
          class:active={auditFilter === 'UNACKED'}
          onclick={() => (auditFilter = 'UNACKED')}
        >
          Unacknowledged
        </button>
      </div>
    </header>

    <div class="audit-list">
      {#if audits.length === 0}
        <div class="empty-state">
          <span class="serif">No Sentinel records match</span>
          <p>
            Trigger interactions in <b>Sandbox</b> or <b>Chats</b> to watch Layer 6 evaluate multi-turn trajectories asynchronously.
          </p>
          {#if auditFilter !== 'ALL'}
            <button type="button" class="btn btn-sm" onclick={() => (auditFilter = 'ALL')}>
              Show all audits
            </button>
          {/if}
        </div>
      {:else}
        {#each audits as a (a.audit_id)}
          {@const sevTone = toneForSeverity(a.severity)}
          <article class="audit-card {sevTone}" in:fly={{ y: 8, duration: 160 }}>
            <div class="a-top">
              <div class="a-badges">
                <span class="mono a-id">{a.audit_id}</span>
                <span class="tag {sevTone} dotted">{a.severity}</span>
                <span class="tag plain mono">{a.engine}</span>
                <span class="tag {a.verdict === 'CLEAN' || a.verdict === 'ALIGNED' ? 'ok' : 'warn'}">
                  {humanize(a.verdict)}
                </span>
                {#if a.intervention !== 'NONE'}
                  <span class="tag danger">
                    <ShieldAlert size={11} /> {humanize(a.intervention)}
                  </span>
                {/if}
              </div>

              <div class="a-right">
                <span class="mono muted a-score">
                  {a.drift_score.toFixed(1)}% · <b class="t-accent">{a.rbi_grade}</b>
                </span>
                <span class="mono muted a-time">{fmtClock(a.created_at)}</span>
                {#if !a.acknowledged}
                  <button type="button" class="btn btn-xs btn-ok" onclick={() => ack(a.audit_id)}>
                    <Check size={11} strokeWidth={2.4} /> Acknowledge
                  </button>
                {:else}
                  <span class="tag plain">Acked</span>
                {/if}
              </div>
            </div>

            <p class="a-summary">{a.summary}</p>

            <div class="a-foot">
              <span class="mono muted">Session: {a.session_id || '—'}</span>
              {#if a.event_id}
                <span class="mono muted">· Event: {a.event_id}</span>
              {/if}
              {#if a.details && Object.keys(a.details).length > 0}
                <details class="a-details">
                  <summary class="mono">Forensic payload</summary>
                  <pre class="code">{JSON.stringify(a.details, null, 2)}</pre>
                </details>
              {/if}
              {#if a.session_id}
                <button
                  type="button"
                  class="btn btn-xs btn-ghost jump-btn"
                  onclick={() => appState.openChatSession(a.session_id!)}
                >
                  <span>Inspect session</span>
                  <ArrowUpRight size={11} />
                </button>
              {/if}
            </div>
          </article>
        {/each}
      {/if}
    </div>
  </section>
</div>

<style>
  .sentinel-page {
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: 100%;
    overflow-y: auto;
    padding-right: 4px;
  }

  .hero-banner {
    padding: 18px 20px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    background:
      radial-gradient(60% 140% at 0% 0%, rgba(var(--accent-rgb), 0.08), transparent 70%),
      var(--card-bg);
  }
  .hb-left {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    max-width: 760px;
  }
  .radar-orb {
    flex: none;
    width: 42px;
    height: 42px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    color: var(--accent);
    background: rgba(var(--accent-rgb), 0.1);
    border: 1px solid rgba(var(--accent-rgb), 0.3);
    box-shadow: 0 0 22px -4px rgba(var(--accent-rgb), 0.45);
  }
  .hb-left h2 {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.015em;
    margin-top: 2px;
  }
  .hb-left p {
    font-size: 12.5px;
    margin-top: 3px;
    line-height: 1.5;
  }
  .hb-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  /* Metrics Row */
  .metrics-row {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }
  .m-card {
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 8px;
  }
  .mc-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .big {
    font-size: 30px;
    font-weight: 600;
    letter-spacing: -0.03em;
    line-height: 1.05;
  }
  .score-bar {
    height: 4px;
    border-radius: 99px;
    background: var(--s-3);
    overflow: hidden;
  }
  .score-fill {
    height: 100%;
    border-radius: 99px;
    background: var(--accent);
    box-shadow: 0 0 10px rgba(var(--accent-rgb), 0.6);
  }
  .sub {
    font-size: 11.5px;
    color: var(--fg-3);
  }
  .chart-wrap {
    margin-top: auto;
  }

  /* Audits Panel */
  .audits-panel {
    flex: 1;
    min-height: 290px;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .audits-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .ah-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .audits-head h3 {
    font-size: 14.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .filter-pills {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: 9px;
    background: var(--well);
    border: 1px solid var(--line);
  }
  .f-pill {
    height: 24px;
    padding: 0 10px;
    border-radius: 7px;
    border: none;
    background: transparent;
    color: var(--fg-3);
    font-size: 11.5px;
    font-weight: 500;
    cursor: pointer;
    transition: color 140ms, background 140ms;
  }
  .f-pill:hover {
    color: var(--fg-2);
  }
  .f-pill.active {
    background: var(--pill-active-bg);
    color: var(--fg);
    box-shadow: var(--pill-active-shadow);
  }

  .audit-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .audit-card {
    position: relative;
    padding: 13px 16px 13px 18px;
    border-radius: 13px;
    background: var(--s-1);
    border: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 8px;
    transition: border-color 150ms;
  }
  .audit-card::before {
    content: '';
    position: absolute;
    left: 0;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
  }
  .audit-card.warn::before { background: var(--warn); }
  .audit-card.danger::before { background: var(--danger); }
  .audit-card:hover {
    border-color: var(--line-2);
  }

  .a-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
  }
  .a-badges {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
  }
  .a-id {
    font-size: 12px;
    font-weight: 600;
    color: var(--fg);
  }
  .a-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .a-score,
  .a-time {
    font-size: 11px;
  }
  .a-summary {
    font-size: 13px;
    color: var(--fg);
    line-height: 1.55;
  }
  .a-foot {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 11px;
  }
  .a-details {
    width: 100%;
    margin-top: 4px;
  }
  .a-details summary {
    font-size: 11px;
    color: var(--fg-3);
    cursor: pointer;
  }
  .a-details pre {
    margin-top: 6px;
    font-size: 11px;
  }
  .jump-btn {
    margin-left: auto;
  }

  @media (max-width: 1100px) {
    .metrics-row {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  /* Minimal Demo Mode adjustments */
  :global(:root[data-demo='true']) .hero-banner p,
  :global(:root[data-demo='true']) .hb-actions button:first-child,
  :global(:root[data-demo='true']) .filter-pills,
  :global(:root[data-demo='true']) .chart-card,
  :global(:root[data-demo='true']) .a-id,
  :global(:root[data-demo='true']) .a-time,
  :global(:root[data-demo='true']) .a-foot,
  :global(:root[data-demo='true']) .audit-card:nth-child(n + 3) {
    display: none;
  }
  :global(:root[data-demo='true']) .sentinel-page {
    gap: 18px;
  }
  :global(:root[data-demo='true']) .metrics-row {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
  }
  :global(:root[data-demo='true']) .m-card {
    padding: 22px 24px;
    gap: 12px;
  }
  :global(:root[data-demo='true']) .big {
    font-size: 48px;
  }
  :global(:root[data-demo='true']) .sub {
    font-size: 13.5px;
  }
  :global(:root[data-demo='true']) .audit-card {
    padding: 18px 22px;
    gap: 10px;
  }
  :global(:root[data-demo='true']) .a-summary {
    font-size: 15.5px;
  }
</style>
