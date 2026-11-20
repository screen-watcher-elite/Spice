<script lang="ts">
  import { appState } from '../lib/stores/appState.svelte';
  import type { TelemetryEvent } from '../lib/api';
  import AnimatedNumber from '../lib/components/AnimatedNumber.svelte';
  import Sparkline from '../lib/components/Sparkline.svelte';
  import PipelineVisualizer from '../lib/components/PipelineVisualizer.svelte';
  import PiiText from '../lib/components/PiiText.svelte';
  import { fly, fade } from 'svelte/transition';
  import {
    toneForEvent,
    toneForLayer,
    humanize,
    fmtUs,
    fmtClock,
    piiSummary,
  } from '../lib/ui';
  import {
    ArrowUpRight,
    Search,
    X,
    Copy,
    Check,
    FlaskConical,
    ShieldCheck,
    ShieldOff,
    Sparkles,
  } from 'lucide-svelte';

  let filterType = $state<string>('ALL');
  let searchQuery = $state<string>('');
  let selectedEvent = $state<TelemetryEvent | null>(null);
  let copiedId = $state<string | null>(null);

  const filters: Array<{ id: string; label: string }> = [
    { id: 'ALL', label: 'All events' },
    { id: 'BLOCKED', label: 'Blocked' },
    { id: 'MASKED', label: 'PII Masked' },
    { id: 'CIRCUIT_TRIP', label: 'Circuit Trip' },
    { id: 'STAFF_OVERRIDE', label: 'Staff Override' },
    { id: 'SENTINEL_ALERT', label: 'Sentinel Alert' },
  ];

  const armed = $derived(appState.shieldStatus?.shield_active ?? true);

  const filteredEvents = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    return appState.events.filter((e) => {
      if (filterType !== 'ALL' && e.event_type !== filterType) return false;
      if (!q) return true;
      return (
        e.event_id.toLowerCase().includes(q) ||
        (e.account_id ?? '').toLowerCase().includes(q) ||
        (e.user_prompt_sanitized ?? '').toLowerCase().includes(q) ||
        (e.decision_source ?? '').toLowerCase().includes(q) ||
        e.event_type.toLowerCase().includes(q)
      );
    });
  });

  const maxLayerUs = $derived.by(() => {
    if (!selectedEvent?.layers?.length) return 1;
    return Math.max(1, ...selectedEvent.layers.map((l) => l.latency_us || 0));
  });

  function copyText(text: string, id: string) {
    navigator.clipboard?.writeText(text);
    copiedId = id;
    setTimeout(() => {
      if (copiedId === id) copiedId = null;
    }, 1500);
  }
</script>

<div class="overview">
  {#if !appState.quickStartDismissed}
    <div class="onboard card">
      <div class="ob-left">
        <span class="tag ok dotted">Interactive Tour</span>
        <span class="ob-text">
          Explore the perimeter in three clicks — fire an adversarial attack, clear a held ₹25,000 refund, or tune live RBI guardrails.
        </span>
      </div>
      <div class="ob-steps">
        <button type="button" class="ob-step" onclick={() => (appState.activeTab = 'sandbox')}>
          <span class="mono ob-num">01</span>
          <span>Attack Sandbox</span>
          <ArrowUpRight size={12} />
        </button>
        <button
          type="button"
          class="ob-step"
          onclick={() => (appState.pendingOverridesCount > 0 ? appState.openOverridesQueue() : (appState.activeTab = 'chats'))}
        >
          <span class="mono ob-num">02</span>
          <span>Maker-Checker ({appState.pendingOverridesCount})</span>
          <ArrowUpRight size={12} />
        </button>
        <button type="button" class="ob-step" onclick={() => (appState.activeTab = 'settings')}>
          <span class="mono ob-num">03</span>
          <span>Policy & Keys</span>
          <ArrowUpRight size={12} />
        </button>
        <button
          type="button"
          class="btn btn-icon btn-sm btn-ghost"
          aria-label="Dismiss quick start"
          onclick={() => (appState.quickStartDismissed = true)}
        >
          <X size={13} />
        </button>
      </div>
    </div>
  {/if}

  <!-- Bento Metric Strip -->
  <section class="bento" aria-label="Perimeter metrics">
    <!-- Hero Latency Card -->
    <div class="card hero-card">
      <div class="hc-top">
        <div>
          <div class="eyebrow">Inline Overhead · L1–L5</div>
          <div class="hero-num">
            <AnimatedNumber value={appState.stats.avg_overhead_ms} decimals={2} />
            <span class="unit serif">ms</span>
          </div>
        </div>
        <div class="pct-cluster mono">
          <div class="pct">
            <span class="p-lbl">P50</span>
            <span class="p-val">{appState.stats.p50_overhead_ms.toFixed(2)}ms</span>
          </div>
          <div class="pct">
            <span class="p-lbl">P95</span>
            <span class="p-val">{appState.stats.p95_overhead_ms.toFixed(2)}ms</span>
          </div>
          <div class="pct">
            <span class="p-lbl">P99</span>
            <span class="p-val t-accent">{appState.stats.p99_overhead_ms.toFixed(2)}ms</span>
          </div>
        </div>
      </div>

      <div class="hc-chart">
        <Sparkline
          values={appState.stats.recent_latencies_ms.length > 1
            ? appState.stats.recent_latencies_ms
            : [0.42, 0.58, 0.51, 0.64, 0.49, appState.stats.avg_overhead_ms || 0.52]}
          color="var(--accent)"
          height={56}
          grid={true}
        />
      </div>

      <div class="hc-foot mono">
        <span>{appState.stats.total_requests.toLocaleString('en-IN')} requests inspected</span>
        <span class="muted">Zero-copy Rust proxy</span>
      </div>
    </div>

    <!-- Threats Neutralized -->
    <button
      type="button"
      class="card stat-card interactive"
      onclick={() => (filterType = filterType === 'BLOCKED' ? 'ALL' : 'BLOCKED')}
    >
      <div class="sc-head">
        <span class="eyebrow">Threats Blocked</span>
        <span class="tag danger">{filterType === 'BLOCKED' ? 'Filtered' : 'L1 · L3'}</span>
      </div>
      <div class="sc-val t-danger">
        <AnimatedNumber value={appState.stats.blocked_attacks} />
      </div>
      <div class="sc-chart">
        <Sparkline
          values={[0, 1, 1, 2, 3, appState.stats.blocked_attacks]}
          color="var(--danger)"
          height={32}
        />
      </div>
      <div class="sc-sub">
        <span>Injections & overrides stopped</span>
      </div>
    </button>

    <!-- PII Tokens Vaulted -->
    <button
      type="button"
      class="card stat-card interactive"
      onclick={() => (filterType = filterType === 'MASKED' ? 'ALL' : 'MASKED')}
    >
      <div class="sc-head">
        <span class="eyebrow">PII Tokens Vaulted</span>
        <span class="tag warn">{filterType === 'MASKED' ? 'Filtered' : 'DPDP 2023'}</span>
      </div>
      <div class="sc-val t-warn">
        <AnimatedNumber value={appState.stats.pii_masked_count} />
      </div>
      <div class="sc-chart">
        <Sparkline
          values={[0, 1, 2, 3, appState.stats.pii_masked_count]}
          color="var(--warn)"
          height={32}
        />
      </div>
      <div class="sc-sub">
        <span>PAN, Aadhaar & cards isolated</span>
      </div>
    </button>

    <!-- Pending Maker-Checker Overrides -->
    <button
      type="button"
      class="card stat-card interactive"
      onclick={() => (appState.pendingOverridesCount > 0 ? appState.openOverridesQueue() : (appState.activeTab = 'chats'))}
    >
      <div class="sc-head">
        <span class="eyebrow">Pending Overrides</span>
        <span class="tag staff">L4 Breaker</span>
      </div>
      <div class="sc-val t-staff">
        <AnimatedNumber value={appState.pendingOverridesCount} />
      </div>
      <div class="sc-meta mono">
        <span>{appState.stats.circuit_trips} trips</span>
        <span>·</span>
        <span>{appState.stats.staff_overrides} resolved</span>
      </div>
      <div class="sc-sub link">
        <span>Open Maker-Checker queue</span>
        <ArrowUpRight size={13} />
      </div>
    </button>

    <!-- RBI Compliance Grade -->
    <button
      type="button"
      class="card stat-card interactive"
      onclick={() => (appState.activeTab = 'sentinel')}
    >
      <div class="sc-head">
        <span class="eyebrow">Compliance Grade</span>
        <span class="tag ok">L6 Sentinel</span>
      </div>
      <div class="sc-val t-accent mono">
        {appState.sentinel?.rbi_compliance_rating || 'AAA'}
      </div>
      <div class="sc-chart">
        <Sparkline
          values={appState.sentinel?.recent_drift_trend || [98.5, 98.8, 99.1]}
          color="var(--accent)"
          height={32}
        />
      </div>
      <div class="sc-sub link">
        <span>
          Alignment {appState.sentinel?.alignment_drift_score?.toFixed(1) || '98.8'}% · {appState.sentinel?.total_audited || 0} audited
        </span>
        <ArrowUpRight size={13} />
      </div>
    </button>
  </section>

  <!-- 6-Layer Security Mesh -->
  <section class="card mesh-box" aria-label="6-Layer security mesh">
    <div class="sec-head">
      <div class="sh-left">
        <span class="eyebrow">Pipeline Architecture</span>
        <div class="sh-title-row">
          <h2>Six-Layer Inline & Async Security Mesh</h2>
          <span class="tag {armed ? 'ok' : 'danger'} dotted">
            {#if armed}<ShieldCheck size={11} /> 6 Layers Enforced{:else}<ShieldOff size={11} /> Perimeter Bypassed{/if}
          </span>
        </div>
      </div>
      <button type="button" class="btn btn-sm" onclick={() => (appState.activeTab = 'sandbox')}>
        <FlaskConical size={13} />
        <span>Simulate Payload</span>
        <ArrowUpRight size={12} />
      </button>
    </div>

    <PipelineVisualizer layers={appState.lastLayers} shieldActive={armed} />
  </section>

  <!-- Live Telemetry Stream -->
  <section class="card stream-box" aria-label="Live telemetry stream">
    <div class="sec-head">
      <div class="sh-left">
        <div class="sh-title-row">
          <h2>Live Forensic Stream</h2>
          <span class="mono count-pill">{filteredEvents.length}</span>
          <span class="sub-hint">Zero plaintext PII — click any transaction for full layer forensics</span>
        </div>
      </div>

      <div class="stream-controls">
        <div class="search-mini">
          <Search size={13} />
          <input
            type="text"
            placeholder="Filter account, payload, ID…"
            bind:value={searchQuery}
            aria-label="Filter telemetry events"
          />
          {#if searchQuery}
            <button type="button" class="clear-btn" onclick={() => (searchQuery = '')} aria-label="Clear search">
              <X size={11} />
            </button>
          {/if}
        </div>

        <div class="filter-pills" role="tablist" aria-label="Event type filter">
          {#each filters as f}
            <button
              type="button"
              role="tab"
              aria-selected={filterType === f.id}
              class="f-pill"
              class:active={filterType === f.id}
              onclick={() => (filterType = f.id)}
            >
              {f.label}
            </button>
          {/each}
        </div>
      </div>
    </div>

    <div class="table-wrap">
      {#if filteredEvents.length === 0}
        <div class="empty-state">
          <span class="serif">Quiet on the perimeter.</span>
          <p>
            No telemetry events match the active filter. Fire a scenario in <b>Sandbox</b> or send a message in <b>Chats</b>.
          </p>
          <div style="display: flex; gap: 8px; margin-top: 4px;">
            {#if filterType !== 'ALL' || searchQuery}
              <button
                type="button"
                class="btn btn-sm"
                onclick={() => {
                  filterType = 'ALL';
                  searchQuery = '';
                }}
              >
                Reset filters
              </button>
            {/if}
            <button type="button" class="btn btn-sm btn-solid" onclick={() => (appState.activeTab = 'sandbox')}>
              Launch Sandbox
            </button>
          </div>
        </div>
      {:else}
        <table>
          <thead>
            <tr>
              <th style="width: 112px;">Time</th>
              <th style="width: 148px;">Verdict</th>
              <th style="width: 128px;">Account</th>
              <th>Sanitized Payload (LLM View)</th>
              <th style="width: 155px;">Engine / Source</th>
              <th class="num" style="width: 96px;">Overhead</th>
            </tr>
          </thead>
          <tbody>
            {#each filteredEvents as ev (ev.event_id)}
              {@const tone = toneForEvent(ev.event_type)}
              {@const piiNote = piiSummary(ev.pii_masked)}
              <tr
                class="ev-row"
                class:selected={selectedEvent?.event_id === ev.event_id}
                onclick={() => (selectedEvent = ev)}
              >
                <td class="mono muted">{fmtClock(ev.iso_time)}</td>
                <td>
                  <span class="tag {tone} dotted">{humanize(ev.event_type)}</span>
                </td>
                <td class="mono dim">{ev.account_id || '—'}</td>
                <td class="payload-cell">
                  <div class="payload-line">
                    <PiiText text={ev.user_prompt_sanitized} />
                  </div>
                  {#if piiNote}
                    <span class="pii-count mono">{piiNote}</span>
                  {/if}
                </td>
                <td class="mono muted src-cell">{ev.decision_source || ev.target_model}</td>
                <td class="mono num t-accent">{ev.overhead_ms.toFixed(2)}ms</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </section>
</div>

<!-- Forensic Slide-Over Inspector -->
{#if selectedEvent}
  {@const evTone = toneForEvent(selectedEvent.event_type)}
  <div class="drawer-backdrop" transition:fade={{ duration: 150 }}>
    <button
      type="button"
      class="backdrop-dismiss"
      aria-label="Close forensic inspector"
      onclick={() => (selectedEvent = null)}
    ></button>
    <div
      class="drawer glass"
      role="dialog"
      aria-modal="true"
      aria-label="Forensic event inspector"
      transition:fly={{ x: 360, duration: 240 }}
    >
      <header class="dr-head">
        <div class="dr-title-wrap">
          <div class="dr-tags">
            <span class="tag {evTone} dotted">{humanize(selectedEvent.event_type)}</span>
            {#if selectedEvent.violation_code}
              <span class="tag warn">{selectedEvent.violation_code}</span>
            {/if}
          </div>
          <h3 class="mono dr-id">{selectedEvent.event_id}</h3>
        </div>
        <div class="dr-actions">
          {#if selectedEvent.session_id}
            <button
              type="button"
              class="btn btn-xs"
              onclick={() => {
                const sid = selectedEvent?.session_id;
                selectedEvent = null;
                if (sid) appState.openChatSession(sid);
              }}
            >
              <span>Open in Chats</span>
              <ArrowUpRight size={12} />
            </button>
          {/if}
          <button
            type="button"
            class="btn btn-icon btn-sm btn-ghost"
            onclick={() => (selectedEvent = null)}
            aria-label="Close inspector"
          >
            <X size={15} />
          </button>
        </div>
      </header>

      <div class="dr-body">
        <div class="kv-grid">
          <div class="kv">
            <span class="eyebrow">Timestamp</span>
            <span class="mono">{selectedEvent.iso_time}</span>
          </div>
          <div class="kv">
            <span class="eyebrow">Account</span>
            <span class="mono">{selectedEvent.account_id || '—'}</span>
          </div>
          <div class="kv">
            <span class="eyebrow">Inline Overhead</span>
            <span class="mono t-accent">{selectedEvent.overhead_ms.toFixed(3)} ms</span>
          </div>
          <div class="kv">
            <span class="eyebrow">Decision Engine</span>
            <span class="mono">{selectedEvent.decision_source || selectedEvent.target_model || '—'}</span>
          </div>
        </div>

        {#if selectedEvent.reason}
          <div class="callout {evTone}">
            <div class="eyebrow">Policy Verdict</div>
            <p>{selectedEvent.reason}</p>
          </div>
        {/if}

        <div class="dr-sec">
          <div class="dr-sec-head">
            <span class="eyebrow">Sanitized Prompt (Upstream LLM View)</span>
            <button
              type="button"
              class="btn btn-xs btn-ghost"
              onclick={() => copyText(selectedEvent!.user_prompt_sanitized, selectedEvent!.event_id)}
            >
              {#if copiedId === selectedEvent.event_id}
                <Check size={11} /> Copied
              {:else}
                <Copy size={11} /> Copy
              {/if}
            </button>
          </div>
          <div class="code prompt-box">
            <PiiText text={selectedEvent.user_prompt_sanitized} />
          </div>
        </div>

        {#if selectedEvent.pii_masked && Object.keys(selectedEvent.pii_masked).length > 0}
          <div class="dr-sec">
            <span class="eyebrow">DPDP Ephemeral Vault Isolations</span>
            <div class="pii-chips">
              {#each Object.entries(selectedEvent.pii_masked) as [kind, count]}
                <span class="tag warn">{count}× {kind}</span>
              {/each}
            </div>
          </div>
        {/if}

        {#if selectedEvent.layers && selectedEvent.layers.length > 0}
          <div class="dr-sec">
            <span class="eyebrow">Layer Execution Breakdown</span>
            <div class="layer-bars">
              {#each selectedEvent.layers as l}
                {@const lt = toneForLayer(l.status)}
                {@const pct = Math.max(4, Math.round(((l.latency_us || 0) / maxLayerUs) * 100))}
                <div class="lbar">
                  <div class="lbar-top">
                    <span class="mono muted">L{l.layer}</span>
                    <span class="lbar-name">{l.name}</span>
                    <span class="mono muted">{fmtUs(l.latency_us)}</span>
                    <span class="tag {lt}">{l.status}</span>
                  </div>
                  <div class="lbar-track">
                    <div class="lbar-fill {lt}" style="width: {pct}%"></div>
                  </div>
                  <div class="lbar-detail">{l.detail}</div>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if selectedEvent.tool_name}
          <div class="dr-sec">
            <span class="eyebrow">Intercepted Tool Call · <span class="mono">{selectedEvent.tool_name}()</span></span>
            <pre class="code">{JSON.stringify(selectedEvent.tool_args, null, 2)}</pre>
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

  /* ---------- Onboarding strip ---------- */
  .onboard {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 14px;
    flex-wrap: wrap;
    background:
      radial-gradient(60% 180% at 0% 50%, rgba(var(--accent-rgb), 0.08), transparent 70%),
      var(--card-bg);
    border-color: rgba(var(--accent-rgb), 0.2);
  }
  .ob-left {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .ob-text {
    font-size: 12.5px;
    color: var(--fg-2);
  }
  .ob-steps {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .ob-step {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    padding: 0 11px;
    border-radius: 8px;
    border: 1px solid var(--line-2);
    background: var(--s-2);
    color: var(--fg);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: border-color 160ms, background 160ms, transform 160ms;
  }
  .ob-step:hover {
    border-color: rgba(var(--accent-rgb), 0.45);
    background: rgba(var(--accent-rgb), 0.08);
    transform: translateY(-1px);
  }
  .ob-num {
    font-size: 10px;
    color: var(--accent);
  }

  /* ---------- Bento KPI row ---------- */
  .bento {
    display: grid;
    grid-template-columns: 1.55fr repeat(4, minmax(0, 1fr));
    gap: 12px;
  }

  .hero-card {
    padding: 16px 18px 14px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 10px;
    background:
      radial-gradient(80% 120% at 100% 0%, rgba(var(--accent-rgb), 0.08), transparent 65%),
      var(--card-bg);
  }
  .hc-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }
  .hero-num {
    font-size: 34px;
    font-weight: 600;
    letter-spacing: -0.035em;
    line-height: 1.05;
    color: var(--fg);
    margin-top: 4px;
    display: flex;
    align-items: baseline;
    gap: 5px;
  }
  .hero-num .unit {
    font-size: 22px;
    font-style: italic;
    color: var(--fg-3);
    font-weight: 400;
  }
  .pct-cluster {
    display: flex;
    gap: 12px;
    padding: 6px 10px;
    border-radius: 9px;
    background: var(--well);
    border: 1px solid var(--line);
  }
  .pct {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .pct + .pct {
    padding-left: 10px;
    border-left: 1px solid var(--line);
  }
  .p-lbl {
    font-size: 9.5px;
    color: var(--fg-3);
    letter-spacing: 0.08em;
  }
  .p-val {
    font-size: 11.5px;
    color: var(--fg-2);
  }
  .hc-chart {
    margin: 2px 0;
  }
  .hc-foot {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: var(--fg-2);
  }

  .stat-card {
    padding: 16px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 8px;
    text-align: left;
  }
  .stat-card.interactive {
    cursor: pointer;
    transition: border-color 180ms var(--ease), transform 180ms var(--ease), background 180ms var(--ease);
  }
  .stat-card.interactive:hover {
    border-color: var(--line-3);
    transform: translateY(-2px);
    background: var(--card-hover-bg);
  }
  .sc-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }
  .sc-val {
    font-size: 30px;
    font-weight: 600;
    letter-spacing: -0.03em;
    line-height: 1.05;
  }
  .sc-chart {
    margin: 2px 0;
  }
  .sc-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--fg-2);
    margin: auto 0;
  }
  .sc-sub {
    font-size: 11.5px;
    color: var(--fg-3);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 4px;
  }
  .sc-sub.link {
    color: var(--fg-2);
  }
  .stat-card.interactive:hover .sc-sub.link {
    color: var(--fg);
  }

  /* ---------- Sections ---------- */
  .mesh-box {
    padding: 18px 20px 20px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .stream-box {
    flex: 1;
    min-height: 290px;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .sec-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
  }
  .sh-left {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .sh-title-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .sec-head h2 {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.015em;
  }
  .count-pill {
    font-size: 11px;
    padding: 1px 7px;
    border-radius: 99px;
    background: var(--s-2);
    border: 1px solid var(--line);
    color: var(--fg-2);
  }
  .sub-hint {
    font-size: 12px;
    color: var(--fg-3);
  }

  .stream-controls {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .search-mini {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    padding: 0 10px;
    border-radius: 8px;
    background: var(--well);
    border: 1px solid var(--line-2);
    color: var(--fg-3);
    width: 220px;
  }
  .search-mini:focus-within {
    border-color: rgba(var(--accent-rgb), 0.5);
    color: var(--fg-2);
  }
  .search-mini input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--fg);
    font: inherit;
    font-size: 12px;
  }
  .search-mini input::placeholder {
    color: var(--fg-4);
  }
  .clear-btn {
    background: none;
    border: none;
    color: var(--fg-3);
    cursor: pointer;
    display: inline-flex;
  }

  .filter-pills {
    display: inline-flex;
    align-items: center;
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
    transition: color 150ms, background 150ms;
  }
  .f-pill:hover {
    color: var(--fg-2);
  }
  .f-pill.active {
    background: var(--pill-active-bg);
    color: var(--fg);
    box-shadow: var(--pill-active-shadow);
  }

  /* ---------- Table ---------- */
  .table-wrap {
    flex: 1;
    overflow: auto;
    margin: 0 -6px;
    padding: 0 6px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  th {
    text-align: left;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--fg-3);
    padding: 8px 10px;
    border-bottom: 1px solid var(--line);
    position: sticky;
    top: 0;
    background: var(--bg-1);
    z-index: 2;
  }
  td {
    padding: 10px 10px;
    border-bottom: 1px solid var(--line);
    vertical-align: middle;
  }
  .num {
    text-align: right;
  }
  .ev-row {
    cursor: pointer;
    transition: background 140ms var(--ease);
  }
  .ev-row:hover {
    background: var(--s-1);
  }
  .ev-row.selected {
    background: rgba(var(--accent-rgb), 0.055);
  }
  .payload-cell {
    max-width: 520px;
  }
  .payload-line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--fg);
  }
  .pii-count {
    display: inline-block;
    font-size: 10px;
    color: var(--warn);
    margin-top: 2px;
  }
  .src-cell {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 160px;
  }

  /* ---------- Forensic Drawer ---------- */
  .drawer-backdrop {
    position: fixed;
    inset: 0;
    z-index: 75;
    background: var(--backdrop-bg);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
    display: flex;
    justify-content: flex-end;
  }
  .backdrop-dismiss {
    flex: 1;
    border: none;
    background: transparent;
    cursor: default;
  }
  .drawer {
    position: relative;
    z-index: 1;
    width: min(500px, 92vw);
    height: 100%;
    border-top: none;
    border-right: none;
    border-bottom: none;
    border-left: 1px solid var(--line-2);
    display: flex;
    flex-direction: column;
    box-shadow: var(--shadow-pop);
  }
  .dr-head {
    padding: 20px 22px 16px;
    border-bottom: 1px solid var(--line);
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
  }
  .dr-title-wrap {
    min-width: 0;
  }
  .dr-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .dr-tags {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .dr-id {
    font-size: 14px;
    font-weight: 500;
    color: var(--fg);
  }
  .dr-body {
    flex: 1;
    overflow-y: auto;
    padding: 18px 22px 24px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .kv-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 10px;
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--s-1);
    border: 1px solid var(--line);
  }
  .kv {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
  }
  .callout {
    padding: 12px 14px;
    border-radius: 12px;
    background: rgba(var(--warn-rgb), 0.07);
    border: 1px solid rgba(var(--warn-rgb), 0.25);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .callout.danger {
    background: rgba(var(--danger-rgb), 0.08);
    border-color: rgba(var(--danger-rgb), 0.28);
  }
  .callout p {
    font-size: 12.5px;
    color: var(--fg);
    line-height: 1.5;
  }
  .dr-sec {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .dr-sec-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .prompt-box {
    line-height: 1.7;
  }
  .pii-chips {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .layer-bars {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .lbar {
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--s-1);
    border: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .lbar-top {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .lbar-name {
    font-weight: 500;
    flex: 1;
  }
  .lbar-track {
    height: 3px;
    border-radius: 99px;
    background: var(--s-3);
    overflow: hidden;
  }
  .lbar-fill {
    height: 100%;
    border-radius: 99px;
    background: var(--accent);
  }
  .lbar-fill.warn { background: var(--warn); }
  .lbar-fill.danger { background: var(--danger); }
  .lbar-fill.mute { background: var(--fg-3); }
  .lbar-detail {
    font-size: 11.5px;
    color: var(--fg-3);
  }

  @media (max-width: 1260px) {
    .bento {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .hero-card {
      grid-column: span 2;
    }
  }

  /* Minimal Demo Mode adjustments */
  :global(:root[data-demo='true']) .onboard,
  :global(:root[data-demo='true']) .pct-cluster,
  :global(:root[data-demo='true']) .hc-chart,
  :global(:root[data-demo='true']) .sc-chart,
  :global(:root[data-demo='true']) .sc-sub,
  :global(:root[data-demo='true']) .hc-foot .muted,
  :global(:root[data-demo='true']) .sub-hint,
  :global(:root[data-demo='true']) .stream-controls,
  :global(:root[data-demo='true']) .sc-meta,
  :global(:root[data-demo='true']) .bento > .stat-card:nth-child(4) {
    display: none;
  }
  :global(:root[data-demo='true']) .overview {
    gap: 20px;
  }
  :global(:root[data-demo='true']) .bento {
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 16px;
  }
  :global(:root[data-demo='true']) .hero-card,
  :global(:root[data-demo='true']) .stat-card {
    padding: 22px 24px;
    gap: 12px;
  }
  :global(:root[data-demo='true']) .hero-num,
  :global(:root[data-demo='true']) .sc-val {
    font-size: 44px;
  }
  :global(:root[data-demo='true']) .sec-head h2 {
    font-size: 18px;
  }
  :global(:root[data-demo='true']) th:nth-child(1),
  :global(:root[data-demo='true']) td:nth-child(1),
  :global(:root[data-demo='true']) th:nth-child(3),
  :global(:root[data-demo='true']) td:nth-child(3),
  :global(:root[data-demo='true']) th:nth-child(5),
  :global(:root[data-demo='true']) td:nth-child(5),
  :global(:root[data-demo='true']) tbody tr:nth-child(n + 5) {
    display: none;
  }
  :global(:root[data-demo='true']) table {
    font-size: 14.5px;
  }
  :global(:root[data-demo='true']) td {
    padding: 13px 14px;
  }
</style>
