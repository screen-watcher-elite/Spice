<script lang="ts">
  import { onMount } from 'svelte';
  import {
    api,
    type ChatMessageRecord,
    type CustomerProfile,
    type CustomerSummary,
    type PendingOverrideRecord,
    type SessionRecord,
  } from '../lib/api';
  import { appState } from '../lib/stores/appState.svelte';
  import ConfirmModal from '../lib/components/ConfirmModal.svelte';
  import ToolTraceCard from '../lib/components/ToolTraceCard.svelte';
  import PiiText from '../lib/components/PiiText.svelte';
  import { fade, fly } from 'svelte/transition';
  import {
    toneForSession,
    friendlySession,
    fmtINR,
    fmtClock,
    relTime,
    initials,
    hueFor,
    piiSummary,
  } from '../lib/ui';
  import {
    Search,
    Plus,
    UserCheck,
    Bot,
    Eye,
    EyeOff,
    ShieldAlert,
    Check,
    X,
    Send,
    Lock,
    Unlock,
    Wallet,
    Trash2,
    Sparkles,
  } from 'lucide-svelte';

  let sessions = $state<SessionRecord[]>([]);
  let customers = $state<CustomerSummary[]>([]);
  let statusFilter = $state<string>('ALL');
  let searchQuery = $state<string>('');
  let selectedSessionId = $state<string | null>(null);

  let activeSession = $state<SessionRecord | null>(null);
  let messages = $state<ChatMessageRecord[]>([]);
  let overrides = $state<PendingOverrideRecord[]>([]);
  let customer = $state<CustomerProfile | null>(null);
  let transactions = $state<Array<Record<string, unknown>>>([]);
  let disputes = $state<Array<Record<string, unknown>>>([]);

  let showNewSessionPicker = $state(false);
  let piiRevealed = $state(false);
  let composerMode = $state<'staff' | 'customer'>('customer');
  let composerText = $state('');
  let sending = $state(false);
  let decidingOverrideId = $state<string | null>(null);
  let applyingManual = $state(false);
  let showMobile360 = $state(false);
  let chatStreamEl = $state<HTMLDivElement | null>(null);

  interface ConfirmSpec {
    title: string;
    message: string;
    confirmLabel: string;
    tone: 'amber' | 'crimson' | 'emerald';
    run: () => Promise<void> | void;
  }
  let pendingConfirm = $state<ConfirmSpec | null>(null);

  const samplePrompts = [
    { label: '₹450 refund', text: 'Please refund ₹450 for my duplicate Swiggy charge.' },
    { label: 'PAN + Aadhaar', text: 'Verify my PAN FDCPC9935C and Aadhaar 8132 1488 2535.' },
    { label: '₹25k high-value', text: 'Please refund ₹25,000 for my Croma transaction immediately.' },
  ];

  const filterTabs = [
    { id: 'ALL', label: 'All' },
    { id: 'NEEDS_OVERRIDE', label: 'Approval' },
    { id: 'STAFF_TAKEOVER', label: 'Staff' },
    { id: 'QUARANTINED', label: 'Locked' },
  ];

  $effect(() => {
    const _len = messages.length;
    if (chatStreamEl && _len > 0) {
      queueMicrotask(() => {
        if (chatStreamEl) chatStreamEl.scrollTop = chatStreamEl.scrollHeight;
      });
    }
  });

  // Maker-Checker edit inputs keyed by override_id
  let ovNotes = $state<Record<string, string>>({});
  let ovAmount = $state<Record<string, string>>({});

  // Manual Staff Action controls
  let manualTab = $state<'refund' | 'kyc' | 'credit' | 'freeze'>('refund');
  let manAmount = $state('450');
  let manReason = $state('Staff verified goodwill adjustment');
  let manKyc = $state('VERIFIED');
  let manCredit = $state('75000');

  const filteredSessions = $derived(
    sessions.filter((s) => {
      const q = searchQuery.trim().toLowerCase();
      if (!q) return true;
      return (
        s.customer_name.toLowerCase().includes(q) ||
        s.account_id.toLowerCase().includes(q) ||
        s.session_id.toLowerCase().includes(q)
      );
    }),
  );

  const pendingOverrides = $derived(overrides.filter((o) => o.status === 'PENDING'));

  function handleActionError(title: string, err: unknown) {
    const msg = (err as Error).message || 'Unexpected error';
    if (msg.includes('Unauthorized') || msg.includes('Admin Key') || msg.includes('401')) {
      appState.requestAdminUnlock(msg);
    } else {
      appState.pushToast('error', title, msg);
    }
  }

  async function loadSessions(selectFirst = false) {
    try {
      const targetFocusId = appState.focusSessionId;
      const targetFilter = appState.focusChatsFilter;
      if (targetFocusId !== null) appState.focusSessionId = null;
      if (targetFilter !== null) {
        statusFilter = targetFilter;
        appState.focusChatsFilter = null;
      }

      let [sRes, cRes] = await Promise.all([
        api.listSessions(statusFilter),
        api.listCustomers(),
      ]);

      if (statusFilter === 'NEEDS_OVERRIDE' && sRes.sessions.length === 0) {
        statusFilter = 'ALL';
        sRes = await api.listSessions('ALL');
      }

      sessions = sRes.sessions;
      customers = cRes.customers;

      if (targetFocusId) {
        await selectSession(targetFocusId);
      } else if (selectFirst && !selectedSessionId && sessions.length > 0) {
        const preferred =
          statusFilter === 'NEEDS_OVERRIDE'
            ? sessions.find((s) => s.pending_overrides_count > 0) ?? sessions[0]
            : sessions[0];
        await selectSession(preferred.session_id);
      } else if (selectedSessionId) {
        await selectSession(selectedSessionId, true);
      }
    } catch (e: unknown) {
      appState.pushToast('error', 'Failed to load sessions', (e as Error).message);
    }
  }

  async function selectSession(id: string, preserveReveal = false) {
    selectedSessionId = id;
    if (!preserveReveal) piiRevealed = false;
    try {
      const detail = await api.getSessionDetail(id);
      activeSession = detail.session;
      messages = detail.messages;
      overrides = detail.overrides;
      customer = detail.customer ?? null;
      transactions = detail.transactions;
      disputes = detail.disputes;

      for (const ov of detail.overrides) {
        if (ovAmount[ov.override_id] === undefined && ov.tool_args?.amount !== undefined) {
          ovAmount[ov.override_id] = String(ov.tool_args.amount);
        }
      }

      if (detail.session.status === 'STAFF_TAKEOVER') {
        composerMode = 'staff';
      }
    } catch (e: unknown) {
      appState.pushToast('error', 'Session load failed', (e as Error).message);
    }
  }

  async function startSessionForCustomer(accId: string) {
    showNewSessionPicker = false;
    try {
      const res = await api.createSession(accId);
      await loadSessions();
      await selectSession(res.session.session_id);
      appState.pushToast('success', 'Customer session started', accId);
    } catch (e: unknown) {
      appState.pushToast('error', 'Could not create session', (e as Error).message);
    }
  }

  async function toggleTakeover() {
    if (!activeSession) return;
    const takingOver = activeSession.status !== 'STAFF_TAKEOVER';
    try {
      await api.toggleTakeover(activeSession.session_id, takingOver, appState.officer);
      composerMode = takingOver ? 'staff' : 'customer';
      appState.pushToast(
        'info',
        takingOver ? 'Chat taken over by staff' : 'Handed back to FinBot AI',
        activeSession.customer_name,
      );
      await loadSessions();
    } catch (e: unknown) {
      handleActionError('Takeover failed', e);
    }
  }

  function toggleQuarantine() {
    if (!activeSession) return;
    const sid = activeSession.session_id;
    const quar = activeSession.status !== 'QUARANTINED';
    pendingConfirm = {
      title: quar ? 'Quarantine customer session?' : 'Lift session quarantine?',
      message: quar
        ? 'Quarantining blocks all automated AI responses on this session until a Compliance Officer clears it.'
        : 'Releasing quarantine allows FinBot AI to resume handling this session.',
      confirmLabel: quar ? 'Quarantine session' : 'Release quarantine',
      tone: quar ? 'crimson' : 'emerald',
      run: async () => {
        try {
          await api.toggleQuarantine(
            sid,
            quar,
            appState.officer,
            quar ? 'Manual compliance quarantine' : 'Cleared by staff officer',
          );
          appState.pushToast('warn', quar ? 'Session quarantined' : 'Quarantine lifted');
          await loadSessions();
        } catch (e: unknown) {
          handleActionError('Quarantine toggle failed', e);
        }
      },
    };
  }

  async function toggleRevealPii() {
    if (!activeSession) return;
    const sid = activeSession.session_id;
    if (piiRevealed) {
      piiRevealed = false;
      await selectSession(sid, true);
      return;
    }
    pendingConfirm = {
      title: 'Reveal unmasked customer PII?',
      message:
        'Unmasking exposes raw Aadhaar, PAN, and phone numbers in this transcript. Requires Admin Key verification and writes an immutable audit log entry.',
      confirmLabel: 'Reveal raw PII',
      tone: 'amber',
      run: async () => {
        try {
          const res = await api.revealPii(sid, appState.officer, 'Staff compliance inspection');
          messages = res.messages;
          piiRevealed = true;
          appState.pushToast('warn', 'PII unmasked (audit logged)', `Officer: ${appState.officer}`);
        } catch (e: unknown) {
          handleActionError('Admin key required to reveal PII', e);
        }
      },
    };
  }

  function requestPurgePii() {
    if (!activeSession) return;
    const sid = activeSession.session_id;
    pendingConfirm = {
      title: 'Purge stored raw PII (DPDP erasure)?',
      message:
        'Permanently replaces all unmasked customer message copies with tokenized placeholders for this session. This action cannot be undone.',
      confirmLabel: 'Purge raw PII',
      tone: 'crimson',
      run: async () => {
        try {
          const res = await api.purgeSessionPii(sid, appState.officer);
          piiRevealed = false;
          appState.pushToast('success', 'Raw PII purged', `${res.purged_messages} message(s) scrubbed`);
          await selectSession(sid);
        } catch (e: unknown) {
          handleActionError('Purge failed', e);
        }
      },
    };
  }

  async function sendMessage(e: Event) {
    e.preventDefault();
    if (!activeSession || !composerText.trim() || sending) return;
    if (activeSession.status === 'QUARANTINED' && composerMode === 'customer') {
      appState.pushToast(
        'warn',
        'Session quarantined',
        'Release quarantine or switch to Staff Reply before sending.',
      );
      return;
    }
    sending = true;
    const text = composerText.trim();
    composerText = '';
    try {
      if (composerMode === 'staff') {
        await api.sendStaffReply(activeSession.session_id, appState.officer, text);
      } else {
        await api.sendCustomerMessage(activeSession.session_id, text);
      }
      await loadSessions();
      await appState.refreshAll();
    } catch (err: unknown) {
      composerText = text;
      handleActionError('Send failed', err);
    } finally {
      sending = false;
    }
  }

  function confirmOverrideDecision(
    ov: PendingOverrideRecord,
    action: 'APPROVE' | 'REJECT' | 'MODIFY_APPROVE',
  ) {
    if (decidingOverrideId) return;
    if (action === 'REJECT') {
      handleOverrideDecision(ov, action);
      return;
    }
    if (action === 'MODIFY_APPROVE' && ov.tool_args?.amount !== undefined) {
      const parsed = parseFloat(ovAmount[ov.override_id] || String(ov.tool_args.amount));
      if (!Number.isFinite(parsed) || parsed < 1) {
        appState.pushToast('error', 'Invalid amount', 'Enter a finite refund amount of ₹1.00 or more.');
        return;
      }
    }
    const amt = action === 'MODIFY_APPROVE' ? ovAmount[ov.override_id] : ov.tool_args?.amount;
    pendingConfirm = {
      title: `Authorize ${ov.tool_name}()?`,
      message: `Approving '${ov.tool_name}' on account ${ov.account_id}${amt !== undefined ? ` (₹${amt})` : ''} as ${appState.officer}. This directly updates the MoneyMax core banking ledger.`,
      confirmLabel: 'Approve & execute ledger update',
      tone: 'emerald',
      run: () => handleOverrideDecision(ov, action),
    };
  }

  async function handleOverrideDecision(
    ov: PendingOverrideRecord,
    action: 'APPROVE' | 'REJECT' | 'MODIFY_APPROVE',
  ) {
    if (decidingOverrideId) return;
    decidingOverrideId = ov.override_id;
    try {
      const notes = ovNotes[ov.override_id] || `Reviewed by ${appState.officer}`;
      let modified_args: Record<string, unknown> | undefined = undefined;
      if (action === 'MODIFY_APPROVE' && ov.tool_args?.amount !== undefined) {
        const parsed = parseFloat(ovAmount[ov.override_id] || String(ov.tool_args.amount));
        if (!Number.isFinite(parsed) || parsed < 1) {
          appState.pushToast('error', 'Invalid amount', 'Enter a finite refund amount of ₹1.00 or more.');
          return;
        }
        modified_args = { ...ov.tool_args, amount: parsed };
      }

      await api.decideOverride(ov.override_id, {
        action,
        officer: appState.officer,
        notes,
        modified_args,
      });
      appState.pushToast(
        action === 'REJECT' ? 'warn' : 'success',
        `Override ${action === 'REJECT' ? 'rejected' : 'approved & executed'}`,
        `${ov.tool_name} on ${ov.account_id}`,
      );
      await loadSessions();
      await appState.refreshAll();
    } catch (e: unknown) {
      handleActionError('Override decision failed', e);
    } finally {
      decidingOverrideId = null;
    }
  }

  async function executeManualStaffAction() {
    if (!customer || applyingManual) return;
    let body: Record<string, unknown> = {
      officer: appState.officer,
      reason: manReason,
      session_id: activeSession?.session_id,
    };
    if (manualTab === 'refund') {
      const amt = parseFloat(manAmount);
      if (!Number.isFinite(amt) || amt < 1) {
        appState.pushToast('error', 'Invalid refund amount', 'Enter a valid amount >= ₹1.00.');
        return;
      }
      body = { ...body, action: 'REFUND', amount: amt };
    } else if (manualTab === 'kyc') {
      body = { ...body, action: 'UPDATE_KYC', kyc_status: manKyc };
    } else if (manualTab === 'credit') {
      const lim = parseFloat(manCredit);
      if (!Number.isFinite(lim) || lim <= 0) {
        appState.pushToast('error', 'Invalid credit limit', 'Enter a positive credit limit.');
        return;
      }
      body = { ...body, action: 'UPDATE_CREDIT_LIMIT', new_limit: lim };
    } else {
      body = { ...body, action: customer.is_frozen ? 'UNFREEZE' : 'FREEZE' };
    }

    applyingManual = true;
    try {
      await api.manualStaffAction(customer.account_id, body);
      appState.pushToast('success', 'Manual staff action applied', customer.account_id);
      await loadSessions();
      await appState.refreshAll();
    } catch (e: unknown) {
      handleActionError('Action failed', e);
    } finally {
      applyingManual = false;
    }
  }

  let lastSeenRevision = appState.revision;

  $effect(() => {
    const fid = appState.focusSessionId;
    const ff = appState.focusChatsFilter;
    if (fid || ff) {
      loadSessions(true);
    }
  });

  $effect(() => {
    const rev = appState.revision;
    if (rev > 0 && rev !== lastSeenRevision) {
      lastSeenRevision = rev;
      loadSessions(false);
    }
  });

  onMount(() => {
    if (!appState.focusSessionId && !appState.focusChatsFilter) {
      loadSessions(true);
    }
  });
</script>

<div class="ops-grid">
  <!-- LEFT PANE: Customer Sessions Rail -->
  <aside class="card pane sessions-pane">
    <header class="pane-head">
      <div class="ph-title">
        <h3>Sessions</h3>
        <span class="mono count-pill">{filteredSessions.length}</span>
      </div>
      <button
        type="button"
        class="btn btn-sm {showNewSessionPicker ? 'btn-ghost' : 'btn-solid'}"
        onclick={() => (showNewSessionPicker = !showNewSessionPicker)}
      >
        {#if showNewSessionPicker}<X size={12} /> Close{:else}<Plus size={12} strokeWidth={2.3} /> New{/if}
      </button>
    </header>

    {#if showNewSessionPicker}
      <div class="picker" transition:fly={{ y: -6, duration: 160 }}>
        <div class="eyebrow">Open session for customer</div>
        <div class="picker-list">
          {#each customers.slice(0, 12) as c}
            <button
              type="button"
              class="picker-item"
              onclick={() => startSessionForCustomer(c.account_id)}
            >
              <span class="av-mini" style="--h: {hueFor(c.account_id)}">{initials(c.full_name)}</span>
              <span class="pi-name">{c.full_name}</span>
              <span class="mono muted pi-acc">{c.account_id}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="rail-controls">
      <div class="search-wrap">
        <Search size={13} />
        <input
          type="text"
          placeholder="Search customer or MMX-AC…"
          bind:value={searchQuery}
          aria-label="Search customer sessions"
        />
        {#if searchQuery}
          <button type="button" class="clear-x" onclick={() => (searchQuery = '')} aria-label="Clear">
            <X size={11} />
          </button>
        {/if}
      </div>

      <div class="seg-tabs" role="tablist">
        {#each filterTabs as ft}
          <button
            type="button"
            role="tab"
            aria-selected={statusFilter === ft.id}
            class="seg-tab"
            class:active={statusFilter === ft.id}
            onclick={() => {
              statusFilter = ft.id;
              loadSessions();
            }}
          >
            {ft.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="session-list">
      {#if filteredSessions.length === 0}
        <div class="empty-state">
          <span class="serif">No sessions found</span>
          <p>Start a new customer session above or clear your filter.</p>
        </div>
      {:else}
        {#each filteredSessions as s (s.session_id)}
          {@const stTone = toneForSession(s.status)}
          <button
            type="button"
            class="sess-card"
            class:selected={selectedSessionId === s.session_id}
            onclick={() => selectSession(s.session_id)}
          >
            <div class="sc-row1">
              <span class="av-mini" style="--h: {hueFor(s.account_id)}">{initials(s.customer_name)}</span>
              <span class="sc-name">{s.customer_name}</span>
              <span class="mono muted sc-time">{relTime(s.updated_at)}</span>
            </div>

            <div class="sc-row2">
              <span class="mono muted">{s.account_id}</span>
              <div class="sc-badges">
                {#if s.pending_overrides_count > 0}
                  <span class="tag warn">{s.pending_overrides_count} held</span>
                {/if}
                <span class="tag {stTone} dotted">{friendlySession(s.status)}</span>
              </div>
            </div>

            {#if s.last_message_preview}
              <div class="sc-prev">
                <PiiText text={s.last_message_preview} />
              </div>
            {/if}
          </button>
        {/each}
      {/if}
    </div>
  </aside>

  <!-- CENTER PANE: Live Conversation + Maker-Checker -->
  <section class="card pane chat-pane">
    {#if activeSession}
      <header class="chat-head">
        <div class="ch-left">
          <span class="av-lg" style="--h: {hueFor(activeSession.account_id)}">
            {initials(activeSession.customer_name)}
          </span>
          <div>
            <div class="ch-title">
              <h3>{activeSession.customer_name}</h3>
              <span class="tag plain mono">{activeSession.account_id}</span>
              <span class="tag {toneForSession(activeSession.status)} dotted">
                {friendlySession(activeSession.status)}
              </span>
              {#if piiRevealed}
                <span class="tag warn">PII UNMASKED</span>
              {/if}
            </div>
            <div class="ch-sub mono">
              <span>{activeSession.session_id}</span>
              {#if activeSession.assigned_officer}
                <span>·</span>
                <span>Assigned: {activeSession.assigned_officer}</span>
              {/if}
            </div>
          </div>
        </div>

        <div class="ch-actions">
          <button
            type="button"
            class="btn btn-sm {activeSession.status === 'STAFF_TAKEOVER' ? 'btn-ok' : 'btn-staff'}"
            onclick={toggleTakeover}
          >
            {#if activeSession.status === 'STAFF_TAKEOVER'}
              <Bot size={13} /> Hand Back to AI
            {:else}
              <UserCheck size={13} /> Take Over Chat
            {/if}
          </button>

          <button
            type="button"
            class="btn btn-sm demo-hide {piiRevealed ? 'btn-warn' : ''}"
            onclick={toggleRevealPii}
            title="Audit-logged DPDP PII unmasking"
          >
            {#if piiRevealed}<EyeOff size={13} /> Hide PII{:else}<Eye size={13} /> Reveal PII{/if}
          </button>

          <button
            type="button"
            class="btn btn-sm demo-hide"
            onclick={requestPurgePii}
            title="DPDP Right-to-Erasure: permanently wipe unmasked copies"
          >
            <Trash2 size={13} /> Purge PII
          </button>

          <button
            type="button"
            class="btn btn-sm demo-hide {activeSession.status === 'QUARANTINED' ? 'btn-ok' : 'btn-danger'}"
            onclick={toggleQuarantine}
          >
            {#if activeSession.status === 'QUARANTINED'}
              <Unlock size={13} /> Release
            {:else}
              <Lock size={13} /> Quarantine
            {/if}
          </button>

          <button
            type="button"
            class="btn btn-sm mobile-360-btn"
            onclick={() => (showMobile360 = true)}
            aria-label="Open Customer 360"
          >
            <Wallet size={13} /> Customer 360
          </button>
        </div>
      </header>

      <!-- Session Status Banners -->
      {#if activeSession.status === 'QUARANTINED'}
        <div class="state-banner danger" transition:fly={{ y: -6, duration: 160 }}>
          <div class="sb-text">
            <Lock size={13} />
            <span>
              <b>Session Quarantined</b> — Automated AI responses are locked until cleared by a compliance officer.
            </span>
          </div>
          <button type="button" class="btn btn-xs btn-ok" onclick={toggleQuarantine}>
            <Unlock size={11} /> Release Quarantine
          </button>
        </div>
      {:else if activeSession.status === 'STAFF_TAKEOVER'}
        <div class="state-banner staff" transition:fly={{ y: -6, duration: 160 }}>
          <div class="sb-text">
            <UserCheck size={13} />
            <span>
              <b>Staff Live Control</b> — FinBot AI is paused while you handle this conversation.
            </span>
          </div>
          <button type="button" class="btn btn-xs btn-ok" onclick={toggleTakeover}>
            <Bot size={11} /> Hand Back to AI
          </button>
        </div>
      {/if}

      <!-- Pending Maker-Checker Approvals Banner -->
      {#if pendingOverrides.length > 0}
        <div class="overrides-stack">
          {#each pendingOverrides as ov (ov.override_id)}
            <div class="override-card" transition:fly={{ y: -8, duration: 200 }}>
              <div class="ov-top">
                <div class="ov-title">
                  <span class="ov-ico"><ShieldAlert size={14} /></span>
                  <span>Maker-Checker Hold —</span>
                  <span class="mono fn">{ov.tool_name}()</span>
                </div>
                <span class="tag warn">{ov.violation_code}</span>
              </div>

              <p class="ov-reason">{ov.reason}</p>

              <div class="ov-controls">
                {#if ov.tool_args?.amount !== undefined}
                  <label class="ov-field amt">
                    <span class="eyebrow">Amount (₹)</span>
                    <input type="number" class="input mono" bind:value={ovAmount[ov.override_id]} />
                  </label>
                {/if}
                <label class="ov-field grow">
                  <span class="eyebrow">Officer Justification</span>
                  <input
                    type="text"
                    class="input"
                    placeholder="Verified merchant dispute receipt…"
                    bind:value={ovNotes[ov.override_id]}
                  />
                </label>
                <div class="ov-btns">
                  <button
                    type="button"
                    class="btn btn-sm btn-solid"
                    disabled={decidingOverrideId === ov.override_id}
                    onclick={() => confirmOverrideDecision(ov, 'APPROVE')}
                  >
                    <Check size={13} strokeWidth={2.4} /> Approve & Execute
                  </button>
                  {#if ov.tool_args?.amount !== undefined}
                    <button
                      type="button"
                      class="btn btn-sm btn-warn"
                      disabled={decidingOverrideId === ov.override_id}
                      onclick={() => confirmOverrideDecision(ov, 'MODIFY_APPROVE')}
                    >
                      Modify & Approve
                    </button>
                  {/if}
                  <button
                    type="button"
                    class="btn btn-sm btn-danger"
                    disabled={decidingOverrideId === ov.override_id}
                    onclick={() => confirmOverrideDecision(ov, 'REJECT')}
                  >
                    <X size={13} /> Reject
                  </button>
                </div>
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Messages Transcript Stream -->
      <div class="messages-stream" bind:this={chatStreamEl}>
        {#each messages as m (m.msg_id)}
          {@const piiNote = piiSummary(m.pii_counts)}
          <div class="msg-row {m.role}" in:fly={{ y: 6, duration: 160 }}>
            <div class="msg-meta">
              <span class="role-lbl">
                {#if m.role === 'customer'}
                  {activeSession.customer_name}
                {:else if m.role === 'assistant'}
                  <Sparkles size={11} /> FinBot AI
                {:else if m.role === 'staff'}
                  {appState.officer}
                {:else}
                  Perimeter Audit
                {/if}
              </span>
              {#if piiNote}
                <span class="tag warn">{piiNote}</span>
              {/if}
              <span class="mono muted time">{fmtClock(m.created_at)}</span>
            </div>

            <div class="msg-bubble">
              {#if piiRevealed && m.content_unmasked}
                {m.content_unmasked}
              {:else}
                <PiiText text={m.content_sanitized} />
              {/if}
            </div>

            {#if m.role === 'assistant'}
              <ToolTraceCard
                layers={m.layer_trace || []}
                tools={m.tool_calls || []}
                decisionSource={m.decision_source}
                sentinelVerdict={m.sentinel_verdict}
                overheadMs={m.overhead_ms}
              />
            {/if}
          </div>
        {/each}
      </div>

      <!-- Dual Composer -->
      <form class="composer" onsubmit={sendMessage}>
        <div class="comp-bar">
          <div class="mode-seg">
            <button
              type="button"
              class="m-pill"
              class:active={composerMode === 'customer'}
              onclick={() => (composerMode = 'customer')}
            >
              Simulate Customer
            </button>
            <button
              type="button"
              class="m-pill staff"
              class:active={composerMode === 'staff'}
              onclick={() => (composerMode = 'staff')}
            >
              Reply as {appState.officer.replace('Officer ', '')}
            </button>
          </div>

          {#if composerMode === 'customer'}
            <div class="sample-pills">
              <span class="eyebrow">Quick test:</span>
              {#each samplePrompts as sp}
                <button
                  type="button"
                  class="sample-pill"
                  onclick={() => (composerText = sp.text)}
                >
                  {sp.label}
                </button>
              {/each}
            </div>
          {/if}
        </div>

        {#if activeSession.status === 'QUARANTINED' && composerMode === 'customer'}
          <div class="quarantine-hint">
            <Lock size={12} />
            <span>
              Customer AI replies are disabled while quarantined — release quarantine above or switch to <b>Reply as {appState.officer.replace('Officer ', '')}</b>.
            </span>
          </div>
        {/if}

        <div class="comp-input-row">
          <input
            type="text"
            class="input"
            placeholder={composerMode === 'staff'
              ? `Send direct staff override reply as ${appState.officer}…`
              : activeSession.status === 'QUARANTINED'
                ? 'Session quarantined — release quarantine or reply as staff…'
                : 'Send customer message through AegisMesh 6-layer gateway…'}
            bind:value={composerText}
            disabled={sending}
          />
          <button
            type="submit"
            class="btn {composerMode === 'staff' ? 'btn-staff' : 'btn-solid'}"
            disabled={sending || !composerText.trim() || (activeSession.status === 'QUARANTINED' && composerMode === 'customer')}
          >
            <Send size={13} />
            <span>{sending ? 'Inspecting…' : 'Send'}</span>
          </button>
        </div>
      </form>
    {:else}
      <div class="empty-state">
        <span class="serif">Select a conversation</span>
        <p>Choose a customer session from the left rail to inspect live messages, tool traces, and Maker-Checker holds.</p>
      </div>
    {/if}
  </section>

  {#if showMobile360}
    <button
      type="button"
      class="mobile-backdrop"
      aria-label="Close Customer 360"
      transition:fade={{ duration: 150 }}
      onclick={() => (showMobile360 = false)}
    ></button>
  {/if}

  <!-- RIGHT PANE: Customer 360 & Direct Ledger Control -->
  <aside class="card pane customer-pane" class:mobile-open={showMobile360}>
    {#if customer}
      <header class="pane-head">
        <div class="ph-title">
          <Wallet size={14} />
          <h3>Customer 360</h3>
        </div>
        <div style="display: flex; align-items: center; gap: 6px;">
          <span class="tag {customer.is_frozen ? 'danger' : 'ok'} dotted">
            {customer.is_frozen ? 'FROZEN' : 'ACTIVE'}
          </span>
          <button
            type="button"
            class="btn btn-icon btn-xs btn-ghost mobile-close-btn"
            onclick={() => (showMobile360 = false)}
            aria-label="Close Customer 360"
          >
            <X size={13} />
          </button>
        </div>
      </header>

      <div class="cust-scroll">
        <!-- Customer Profile Hero -->
        <div class="c-profile">
          <div class="cp-top">
            <span class="av-lg" style="--h: {hueFor(customer.account_id)}">
              {initials(customer.full_name)}
            </span>
            <div class="cp-id">
              <div class="cp-name">{customer.full_name}</div>
              <div class="mono muted cp-vpa">{customer.account_id} · {customer.upi_vpa}</div>
            </div>
          </div>

          <div class="ledger-grid">
            <div class="l-cell">
              <span class="eyebrow">Ledger Balance</span>
              <span class="l-val mono t-accent">{fmtINR(customer.balance, 2)}</span>
            </div>
            <div class="l-cell">
              <span class="eyebrow">Credit Limit</span>
              <span class="l-val mono">{fmtINR(customer.credit_limit)}</span>
            </div>
            <div class="l-cell">
              <span class="eyebrow">KYC State</span>
              <div>
                <span class="tag {customer.kyc_status === 'VERIFIED' ? 'ok' : customer.kyc_status === 'FLAGGED' ? 'danger' : 'warn'}">
                  {customer.kyc_status}
                </span>
              </div>
            </div>
            <div class="l-cell">
              <span class="eyebrow">CIBIL / Risk</span>
              <span class="mono l-sub">{customer.cibil_score} · {customer.risk_tier}</span>
            </div>
          </div>
        </div>

        <!-- Manual Staff Ledger Override -->
        <div class="manual-card">
          <div class="mc-head">
            <span class="eyebrow">Direct Staff Ledger Override</span>
          </div>

          <div class="seg-tabs">
            <button type="button" class="seg-tab" class:active={manualTab === 'refund'} onclick={() => (manualTab = 'refund')}>Refund</button>
            <button type="button" class="seg-tab" class:active={manualTab === 'kyc'} onclick={() => (manualTab = 'kyc')}>KYC</button>
            <button type="button" class="seg-tab" class:active={manualTab === 'credit'} onclick={() => (manualTab = 'credit')}>Credit</button>
            <button type="button" class="seg-tab" class:active={manualTab === 'freeze'} onclick={() => (manualTab = 'freeze')}>Hold</button>
          </div>

          {#if manualTab === 'refund'}
            <label class="field">
              <span class="eyebrow">Credit Amount (₹)</span>
              <input type="number" class="input mono" placeholder="Amount ₹" bind:value={manAmount} />
            </label>
          {:else if manualTab === 'kyc'}
            <label class="field">
              <span class="eyebrow">New KYC State</span>
              <select class="select" bind:value={manKyc}>
                <option value="VERIFIED">VERIFIED</option>
                <option value="PENDING_REVIEW">PENDING_REVIEW</option>
                <option value="FLAGGED">FLAGGED</option>
                <option value="REJECTED">REJECTED</option>
              </select>
            </label>
          {:else if manualTab === 'credit'}
            <label class="field">
              <span class="eyebrow">New Credit Limit (₹)</span>
              <input type="number" class="input mono" placeholder="New limit ₹" bind:value={manCredit} />
            </label>
          {:else}
            <div class="freeze-note">
              Account is currently <b>{customer.is_frozen ? 'FROZEN' : 'ACTIVE'}</b>.
            </div>
          {/if}

          <label class="field">
            <span class="eyebrow">Audit Justification</span>
            <input type="text" class="input" placeholder="Audit justification…" bind:value={manReason} />
          </label>

          <button
            type="button"
            class="btn btn-sm {manualTab === 'freeze' && !customer.is_frozen ? 'btn-danger' : 'btn-warn'}"
            style="width: 100%;"
            disabled={applyingManual}
            onclick={executeManualStaffAction}
          >
            {#if applyingManual}<span class="spinner"></span>{/if}
            {manualTab === 'freeze'
              ? customer.is_frozen
                ? 'Unfreeze Account'
                : 'Freeze Account'
              : `Execute Staff ${manualTab.toUpperCase()}`}
          </button>
        </div>

        <!-- Open Disputes -->
        <div class="sub-list">
          <div class="sl-head">
            <span class="eyebrow">Disputes</span>
            <span class="mono muted">{disputes.length}</span>
          </div>
          {#if disputes.length === 0}
            <div class="muted" style="font-size: 11.5px;">No open disputes</div>
          {:else}
            {#each disputes.slice(0, 4) as d}
              <div class="sl-item">
                <div>
                  <div class="mono" style="font-size: 11.5px;">{d.dispute_id}</div>
                  <div class="muted" style="font-size: 11px;">Claimed {fmtINR(d.claimed_amount)}</div>
                </div>
                <span class="tag {d.status === 'OPEN' ? 'warn' : 'ok'}">{d.status}</span>
              </div>
            {/each}
          {/if}
        </div>

        <!-- Recent Ledger Transactions -->
        <div class="sub-list">
          <div class="sl-head">
            <span class="eyebrow">Recent Ledger Entries</span>
            <span class="mono muted">{transactions.length}</span>
          </div>
          {#each transactions.slice(0, 6) as t}
            <div class="sl-item">
              <div style="min-width: 0;">
                <div class="mono" style="font-size: 11.5px;">{t.txn_id}</div>
                <div class="muted ellip" style="font-size: 11px;">{t.counterparty}</div>
              </div>
              <div class="mono" style="text-align: right;">
                <div class={t.direction === 'CREDIT' ? 't-accent' : ''}>
                  {t.direction === 'CREDIT' ? '+' : '-'}{fmtINR(t.amount)}
                </div>
                <div class="muted" style="font-size: 10px;">{t.status}</div>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <div class="empty-state">
        <span class="serif">Customer 360</span>
        <p>Ledger balances, KYC state, disputes, and staff override controls appear here.</p>
      </div>
    {/if}
  </aside>
</div>

<ConfirmModal
  open={Boolean(pendingConfirm)}
  title={pendingConfirm?.title || ''}
  message={pendingConfirm?.message || ''}
  confirmLabel={pendingConfirm?.confirmLabel || 'Confirm'}
  tone={pendingConfirm?.tone || 'amber'}
  onCancel={() => (pendingConfirm = null)}
  onConfirm={async () => {
    const fn = pendingConfirm?.run;
    pendingConfirm = null;
    if (fn) await fn();
  }}
/>

<style>
  .ops-grid {
    display: grid;
    grid-template-columns: 286px minmax(0, 1fr) 320px;
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }
  .pane-head {
    padding: 13px 16px;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .ph-title {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--fg);
  }
  .ph-title h3 {
    font-size: 13.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .count-pill {
    font-size: 10.5px;
    padding: 1px 6px;
    border-radius: 99px;
    background: var(--s-2);
    border: 1px solid var(--line);
    color: var(--fg-2);
  }

  /* Avatars */
  .av-mini {
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    font-size: 9.5px;
    font-weight: 600;
    color: hsl(var(--h, 90) 80% var(--av-fg-l));
    background: hsl(var(--h, 90) 45% var(--av-bg-l));
    border: 1px solid hsl(var(--h, 90) 55% var(--av-bd-l));
  }
  .av-lg {
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    font-size: 12px;
    font-weight: 600;
    color: hsl(var(--h, 90) 85% var(--av-fg-l));
    background: hsl(var(--h, 90) 45% var(--av-bg-l));
    border: 1px solid hsl(var(--h, 90) 55% var(--av-bd-l));
  }

  /* New session picker */
  .picker {
    margin: 10px 12px 0;
    padding: 10px;
    border-radius: 12px;
    background: var(--well);
    border: 1px solid var(--line-2);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .picker-list {
    max-height: 185px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .picker-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 8px;
    border: 1px solid transparent;
    background: transparent;
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    transition: background 140ms, border-color 140ms;
  }
  .picker-item:hover {
    background: var(--s-2);
    border-color: var(--line-2);
  }
  .pi-name {
    flex: 1;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pi-acc {
    font-size: 10.5px;
  }

  /* Left rail controls */
  .rail-controls {
    padding: 10px 12px 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-bottom: 1px solid var(--line);
  }
  .search-wrap {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 10px;
    border-radius: 8px;
    background: var(--well);
    border: 1px solid var(--line-2);
    color: var(--fg-3);
  }
  .search-wrap:focus-within {
    border-color: rgba(var(--accent-rgb), 0.5);
  }
  .search-wrap input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--fg);
    font: inherit;
    font-size: 12px;
  }
  .search-wrap input::placeholder {
    color: var(--fg-4);
  }
  .clear-x {
    background: none;
    border: none;
    color: var(--fg-3);
    cursor: pointer;
    display: inline-flex;
  }

  .seg-tabs {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--well);
    border: 1px solid var(--line);
  }
  .seg-tab {
    height: 24px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--fg-3);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    transition: color 140ms, background 140ms;
  }
  .seg-tab:hover {
    color: var(--fg-2);
  }
  .seg-tab.active {
    background: var(--pill-active-bg);
    color: var(--fg);
    box-shadow: var(--pill-active-shadow);
  }

  .session-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .sess-card {
    text-align: left;
    padding: 10px 11px;
    border-radius: 11px;
    background: var(--s-1);
    border: 1px solid var(--line);
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 6px;
    transition: border-color 150ms var(--ease), background 150ms var(--ease);
  }
  .sess-card:hover {
    border-color: var(--line-2);
    background: var(--s-2);
  }
  .sess-card.selected {
    border-color: rgba(var(--accent-rgb), 0.45);
    background:
      radial-gradient(120% 120% at 0% 0%, rgba(var(--accent-rgb), 0.09), transparent 70%),
      var(--s-2);
  }
  .sc-row1 {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sc-name {
    flex: 1;
    font-size: 12.5px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sc-time {
    font-size: 10.5px;
  }
  .sc-row2 {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    font-size: 11px;
  }
  .sc-badges {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .sc-prev {
    font-size: 11.5px;
    color: var(--fg-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ---------- Center Chat ---------- */
  .chat-head {
    padding: 12px 18px;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .ch-left {
    display: flex;
    align-items: center;
    gap: 11px;
  }
  .ch-title {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .ch-title h3 {
    font-size: 14.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .ch-sub {
    display: flex;
    gap: 6px;
    font-size: 11px;
    color: var(--fg-3);
    margin-top: 2px;
  }
  .ch-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  /* Maker-Checker Banner */
  .overrides-stack {
    padding: 10px 16px 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .override-card {
    padding: 12px 14px;
    border-radius: 13px;
    background:
      radial-gradient(90% 140% at 0% 0%, rgba(var(--warn-rgb), 0.12), transparent 70%),
      var(--s-1);
    border: 1px solid rgba(var(--warn-rgb), 0.38);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .ov-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .ov-title {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    font-weight: 500;
  }
  .ov-ico {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    background: rgba(var(--warn-rgb), 0.15);
    color: var(--warn);
  }
  .ov-title .fn {
    color: var(--warn);
  }
  .ov-reason {
    font-size: 12px;
    color: var(--fg-2);
  }
  .ov-controls {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    flex-wrap: wrap;
  }
  .ov-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .ov-field.amt {
    width: 130px;
  }
  .ov-field.grow {
    flex: 1;
    min-width: 170px;
  }
  .ov-field .input {
    height: 30px;
    font-size: 12px;
  }
  .ov-btns {
    display: flex;
    gap: 6px;
  }

  /* Messages Stream */
  .messages-stream {
    flex: 1;
    overflow-y: auto;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .msg-row {
    display: flex;
    flex-direction: column;
    max-width: 85%;
  }
  .msg-row.customer {
    align-self: flex-start;
  }
  .msg-row.assistant,
  .msg-row.staff {
    align-self: flex-end;
  }
  .msg-row.system {
    align-self: center;
    max-width: 92%;
  }
  .msg-meta {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-bottom: 4px;
    font-size: 11px;
  }
  .msg-row.assistant .msg-meta,
  .msg-row.staff .msg-meta {
    justify-content: flex-end;
  }
  .role-lbl {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-weight: 500;
    color: var(--fg-2);
  }
  .time {
    font-size: 10px;
  }
  .msg-bubble {
    padding: 10px 14px;
    border-radius: 13px;
    font-size: 13px;
    line-height: 1.55;
    background: var(--s-2);
    border: 1px solid var(--line);
    color: var(--fg);
    white-space: pre-wrap;
  }
  .msg-row.customer .msg-bubble {
    border-top-left-radius: 4px;
  }
  .msg-row.assistant .msg-bubble {
    border-top-right-radius: 4px;
    background:
      radial-gradient(120% 140% at 100% 0%, rgba(var(--accent-rgb), 0.07), transparent 70%),
      var(--s-1);
    border-color: rgba(var(--accent-rgb), 0.22);
  }
  .msg-row.staff .msg-bubble {
    border-top-right-radius: 4px;
    background: rgba(var(--staff-rgb), 0.09);
    border-color: rgba(var(--staff-rgb), 0.32);
  }
  .msg-row.system .msg-bubble {
    background: var(--s-1);
    border-style: dashed;
    font-size: 11.5px;
    color: var(--fg-2);
    text-align: center;
    padding: 7px 14px;
  }

  /* Composer */
  .composer {
    border-top: 1px solid var(--line);
    padding: 10px 16px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--well);
  }
  .comp-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
  }
  .mode-seg {
    display: inline-flex;
    padding: 2px;
    border-radius: 8px;
    background: var(--well);
    border: 1px solid var(--line);
  }
  .m-pill {
    height: 23px;
    padding: 0 10px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: var(--fg-3);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    transition: all 140ms;
  }
  .m-pill.active {
    background: rgba(var(--accent-rgb), 0.14);
    color: var(--accent);
  }
  .m-pill.staff.active {
    background: rgba(var(--staff-rgb), 0.16);
    color: var(--staff);
  }
  .sample-pills {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
  }
  .sample-pill {
    height: 22px;
    padding: 0 9px;
    font-size: 10.5px;
    border-radius: 99px;
    border: 1px solid var(--line-2);
    background: var(--s-1);
    color: var(--fg-2);
    cursor: pointer;
    transition: border-color 140ms, color 140ms, background 140ms;
  }
  .sample-pill:hover {
    border-color: rgba(var(--accent-rgb), 0.45);
    color: var(--accent);
    background: rgba(var(--accent-rgb), 0.06);
  }
  .comp-input-row {
    display: flex;
    gap: 8px;
  }

  /* ---------- Right Customer 360 ---------- */
  .cust-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 12px 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .c-profile {
    padding: 13px;
    border-radius: 13px;
    background: var(--s-1);
    border: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .cp-top {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .cp-id {
    min-width: 0;
  }
  .cp-name {
    font-size: 14px;
    font-weight: 600;
  }
  .cp-vpa {
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ledger-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }
  .l-cell {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .l-val {
    font-size: 14px;
    font-weight: 600;
  }
  .l-sub {
    font-size: 12px;
    color: var(--fg-2);
  }

  .manual-card {
    padding: 13px;
    border-radius: 13px;
    background: var(--s-1);
    border: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field .input,
  .field .select {
    height: 32px;
    font-size: 12px;
  }
  .freeze-note {
    font-size: 12px;
    color: var(--fg-2);
    padding: 4px 0;
  }

  .sub-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .sl-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 11px;
  }
  .sl-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: 9px;
    background: var(--s-1);
    border: 1px solid var(--line);
  }
  .ellip {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 145px;
  }

  /* State banners & mobile 360 */
  .state-banner {
    margin: 10px 16px 0;
    padding: 9px 13px;
    border-radius: 11px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 12px;
  }
  .state-banner.danger {
    background: rgba(var(--danger-rgb), 0.1);
    border: 1px solid rgba(var(--danger-rgb), 0.34);
    color: var(--fg);
  }
  .state-banner.staff {
    background: rgba(var(--staff-rgb), 0.1);
    border: 1px solid rgba(var(--staff-rgb), 0.34);
    color: var(--fg);
  }
  .sb-text {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .state-banner.danger .sb-text :global(svg) {
    color: var(--danger);
    flex: none;
  }
  .state-banner.staff .sb-text :global(svg) {
    color: var(--staff);
    flex: none;
  }
  .quarantine-hint {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 10px;
    border-radius: 8px;
    background: rgba(var(--danger-rgb), 0.08);
    border: 1px solid rgba(var(--danger-rgb), 0.25);
    color: var(--danger);
    font-size: 11.5px;
  }
  .mobile-360-btn,
  .mobile-close-btn,
  .mobile-backdrop {
    display: none;
  }

  @media (max-width: 1200px) {
    .ops-grid {
      grid-template-columns: 250px minmax(0, 1fr);
    }
    .mobile-360-btn,
    .mobile-close-btn {
      display: inline-flex;
    }
    .mobile-backdrop {
      display: block;
      position: fixed;
      inset: 0;
      z-index: 60;
      border: none;
      background: var(--backdrop-bg);
      backdrop-filter: blur(4px);
      -webkit-backdrop-filter: blur(4px);
    }
    .customer-pane {
      display: none;
    }
    .customer-pane.mobile-open {
      display: flex;
      position: fixed;
      top: 68px;
      right: 16px;
      bottom: 16px;
      width: min(350px, 90vw);
      z-index: 65;
      background: var(--glass-bg);
      backdrop-filter: blur(18px);
      box-shadow: var(--shadow-pop);
    }
  }

  /* Minimal Demo Mode adjustments */
  :global(:root[data-demo='true']) .demo-hide,
  :global(:root[data-demo='true']) .rail-controls,
  :global(:root[data-demo='true']) .sc-prev,
  :global(:root[data-demo='true']) .sc-time,
  :global(:root[data-demo='true']) .ch-sub,
  :global(:root[data-demo='true']) .sample-pills,
  :global(:root[data-demo='true']) .ov-field.grow,
  :global(:root[data-demo='true']) .customer-pane:not(.mobile-open) {
    display: none;
  }
  :global(:root[data-demo='true']) .ops-grid {
    grid-template-columns: 248px minmax(0, 1fr);
    gap: 16px;
  }
  :global(:root[data-demo='true']) .mobile-360-btn,
  :global(:root[data-demo='true']) .mobile-close-btn {
    display: inline-flex;
  }
  :global(:root[data-demo='true']) .mobile-backdrop {
    display: block;
    position: fixed;
    inset: 0;
    z-index: 60;
    border: none;
    background: var(--backdrop-bg);
    backdrop-filter: blur(4px);
    -webkit-backdrop-filter: blur(4px);
  }
  :global(:root[data-demo='true']) .customer-pane.mobile-open {
    display: flex;
    position: fixed;
    top: 68px;
    right: 16px;
    bottom: 16px;
    width: min(350px, 90vw);
    z-index: 65;
    background: var(--glass-bg);
    backdrop-filter: blur(18px);
    box-shadow: var(--shadow-pop);
  }
  :global(:root[data-demo='true']) .ch-title h3 {
    font-size: 20px;
  }
  :global(:root[data-demo='true']) .sc-name {
    font-size: 14.5px;
    font-weight: 600;
  }
  :global(:root[data-demo='true']) .msg-bubble {
    font-size: 16px;
    padding: 15px 18px;
  }
  :global(:root[data-demo='true']) .ov-title {
    font-size: 17px;
    font-weight: 600;
  }
  :global(:root[data-demo='true']) .ov-reason {
    font-size: 14.5px;
  }
  :global(:root[data-demo='true']) .ov-field.amt .input {
    height: 38px;
    font-size: 17px;
    font-weight: 700;
  }
  :global(:root[data-demo='true']) .ov-btns .btn {
    height: 38px;
    font-size: 13.5px;
    padding: 0 16px;
  }
</style>
