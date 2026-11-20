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
  import { fade, fly } from 'svelte/transition';
  import {
    Search,
    Plus,
    UserCheck,
    Bot,
    Eye,
    EyeOff,
    ShieldAlert,
    CheckCircle2,
    XCircle,
    Send,
    Lock,
    Unlock,
    Wallet,
    Trash2,
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
    { label: 'Ask ₹450 refund', text: 'Please refund ₹450 for my duplicate Swiggy charge.' },
    { label: 'Share PAN & Aadhaar', text: 'Verify my PAN FDCPC9935C and Aadhaar 8132 1488 2535.' },
    { label: 'Request ₹25k refund', text: 'Please refund ₹25,000 for my Croma transaction immediately.' },
  ];

  $effect(() => {
    const _len = messages.length;
    if (chatStreamEl && _len > 0) {
      queueMicrotask(() => {
        if (chatStreamEl) {
          chatStreamEl.scrollTop = chatStreamEl.scrollHeight;
        }
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

  async function loadSessions(selectFirst = false) {
    try {
      const [sRes, cRes] = await Promise.all([
        api.listSessions(statusFilter),
        api.listCustomers(),
      ]);
      sessions = sRes.sessions;
      customers = cRes.customers;
      if (selectFirst && !selectedSessionId && sessions.length > 0) {
        await selectSession(sessions[0].session_id);
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
      appState.pushToast('success', 'New Customer Session Started', accId);
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
        takingOver ? 'Chat Taken Over by Staff' : 'Handed Back to FinBot AI',
        activeSession.customer_name,
      );
      await loadSessions();
    } catch (e: unknown) {
      appState.pushToast('error', 'Takeover failed', (e as Error).message);
    }
  }

  function toggleQuarantine() {
    if (!activeSession) return;
    const sid = activeSession.session_id;
    const quar = activeSession.status !== 'QUARANTINED';
    pendingConfirm = {
      title: quar ? 'Quarantine Customer Session?' : 'Lift Session Quarantine?',
      message: quar
        ? 'Quarantining blocks all automated AI responses on this session until a Compliance Officer clears it.'
        : 'Releasing quarantine allows FinBot AI to resume handling this session.',
      confirmLabel: quar ? 'Quarantine Session' : 'Release Quarantine',
      tone: quar ? 'crimson' : 'emerald',
      run: async () => {
        try {
          await api.toggleQuarantine(
            sid,
            quar,
            appState.officer,
            quar ? 'Manual compliance quarantine' : 'Cleared by staff officer',
          );
          appState.pushToast('warn', quar ? 'Session Quarantined' : 'Quarantine Lifted');
          await loadSessions();
        } catch (e: unknown) {
          appState.pushToast('error', 'Quarantine toggle failed', (e as Error).message);
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
      title: 'Reveal Unmasked Customer PII?',
      message:
        'Unmasking exposes raw Aadhaar, PAN, and phone numbers in this transcript. This action requires Admin Key verification and writes a permanent audit log entry.',
      confirmLabel: 'Reveal Raw PII',
      tone: 'amber',
      run: async () => {
        try {
          const res = await api.revealPii(sid, appState.officer, 'Staff compliance inspection');
          messages = res.messages;
          piiRevealed = true;
          appState.pushToast('warn', 'PII Unmasked (Audit Logged)', `Officer: ${appState.officer}`);
        } catch (e: unknown) {
          appState.pushToast('error', 'Admin Key Required to Reveal PII', (e as Error).message);
        }
      },
    };
  }

  function requestPurgePii() {
    if (!activeSession) return;
    const sid = activeSession.session_id;
    pendingConfirm = {
      title: 'Purge Stored Raw PII (DPDP Erasure)?',
      message:
        'Permanently replaces all unmasked customer message copies with tokenized placeholders for this session. Cannot be undone.',
      confirmLabel: 'Purge Raw PII',
      tone: 'crimson',
      run: async () => {
        try {
          const res = await api.purgeSessionPii(sid, appState.officer);
          piiRevealed = false;
          appState.pushToast('success', 'Raw PII Purged', `${res.purged_messages} message(s) scrubbed`);
          await selectSession(sid);
        } catch (e: unknown) {
          appState.pushToast('error', 'Purge failed', (e as Error).message);
        }
      },
    };
  }

  async function sendMessage(e: Event) {
    e.preventDefault();
    if (!activeSession || !composerText.trim() || sending) return;
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
      appState.pushToast('error', 'Send failed', (err as Error).message);
    } finally {
      sending = false;
    }
  }

  function confirmOverrideDecision(
    ov: PendingOverrideRecord,
    action: 'APPROVE' | 'REJECT' | 'MODIFY_APPROVE',
  ) {
    if (action === 'REJECT') {
      handleOverrideDecision(ov, action);
      return;
    }
    const amt = action === 'MODIFY_APPROVE' ? ovAmount[ov.override_id] : ov.tool_args?.amount;
    pendingConfirm = {
      title: `Execute Maker-Checker ${ov.tool_name}?`,
      message: `Authorizing '${ov.tool_name}' on account ${ov.account_id}${amt !== undefined ? ` (₹${amt})` : ''} as ${appState.officer}. This directly updates the MoneyMax core banking ledger.`,
      confirmLabel: 'Approve & Execute Ledger Update',
      tone: 'emerald',
      run: () => handleOverrideDecision(ov, action),
    };
  }

  async function handleOverrideDecision(
    ov: PendingOverrideRecord,
    action: 'APPROVE' | 'REJECT' | 'MODIFY_APPROVE',
  ) {
    try {
      const notes = ovNotes[ov.override_id] || `Reviewed by ${appState.officer}`;
      let modified_args: Record<string, unknown> | undefined = undefined;
      if (action === 'MODIFY_APPROVE' && ov.tool_args?.amount !== undefined) {
        const parsed = parseFloat(ovAmount[ov.override_id] || String(ov.tool_args.amount));
        if (!Number.isFinite(parsed) || parsed <= 0) {
          appState.pushToast('error', 'Invalid Amount', 'Enter a positive numeric amount.');
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
        `Override ${action === 'REJECT' ? 'Rejected' : 'Approved & Executed'}`,
        `${ov.tool_name} on ${ov.account_id}`,
      );
      await loadSessions();
      await appState.refreshAll();
    } catch (e: unknown) {
      appState.pushToast('error', 'Override decision failed', (e as Error).message);
    }
  }

  async function executeManualStaffAction() {
    if (!customer) return;
    try {
      let body: Record<string, unknown> = {
        officer: appState.officer,
        reason: manReason,
        session_id: activeSession?.session_id,
      };
      if (manualTab === 'refund') {
        body = { ...body, action: 'REFUND', amount: parseFloat(manAmount) };
      } else if (manualTab === 'kyc') {
        body = { ...body, action: 'UPDATE_KYC', kyc_status: manKyc };
      } else if (manualTab === 'credit') {
        body = { ...body, action: 'UPDATE_CREDIT_LIMIT', new_limit: parseFloat(manCredit) };
      } else {
        body = { ...body, action: customer.is_frozen ? 'UNFREEZE' : 'FREEZE' };
      }
      await api.manualStaffAction(customer.account_id, body);
      appState.pushToast('success', 'Manual Staff Action Applied', customer.account_id);
      await loadSessions();
      await appState.refreshAll();
    } catch (e: unknown) {
      appState.pushToast('error', 'Action Failed', (e as Error).message);
    }
  }

  function statusBadgeClass(st: string): string {
    switch (st) {
      case 'NEEDS_OVERRIDE':
        return 'chip-amber';
      case 'STAFF_TAKEOVER':
        return 'chip-violet';
      case 'QUARANTINED':
        return 'chip-crimson';
      default:
        return 'chip-emerald';
    }
  }

  function friendlyStatus(st: string): string {
    switch (st) {
      case 'NEEDS_OVERRIDE':
        return 'Needs Approval';
      case 'STAFF_TAKEOVER':
        return 'Staff Replying';
      case 'QUARANTINED':
        return 'Locked';
      case 'AI_ACTIVE':
        return 'AI Handling';
      default:
        return st.replace('_', ' ');
    }
  }

  onMount(() => {
    loadSessions(true);
  });
</script>

<div class="ops-grid" in:fade={{ duration: 160 }}>
  <!-- LEFT PANE: Customer Sessions -->
  <aside class="panel pane sessions-pane">
    <div class="pane-head">
      <h3>Customer Chats</h3>
      <button type="button" class="btn btn-sm btn-primary" onclick={() => (showNewSessionPicker = !showNewSessionPicker)}>
        <Plus size={13} /> New
      </button>
    </div>

    {#if showNewSessionPicker}
      <div class="picker panel-elevated" transition:fly={{ y: -6, duration: 140 }}>
        <div class="picker-title">Start session as customer:</div>
        <div class="picker-list">
          {#each customers.slice(0, 12) as c}
            <button type="button" class="picker-item" onclick={() => startSessionForCustomer(c.account_id)}>
              <span>{c.full_name}</span>
              <span class="mono muted">{c.account_id}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="search-box">
      <Search size={13} class="s-icon" />
      <input
        type="text"
        class="input s-input"
        placeholder="Search customer or MMX-AC..."
        bind:value={searchQuery}
      />
    </div>

    <div class="status-tabs">
      {#each ['ALL', 'NEEDS_OVERRIDE', 'STAFF_TAKEOVER', 'QUARANTINED'] as st}
        <button
          type="button"
          class="s-tab"
          class:active={statusFilter === st}
          onclick={() => {
            statusFilter = st;
            loadSessions();
          }}
        >
          {st === 'ALL' ? 'All' : friendlyStatus(st)}
        </button>
      {/each}
    </div>

    <div class="session-list">
      {#each filteredSessions as s (s.session_id)}
        <button
          type="button"
          class="sess-card"
          class:selected={selectedSessionId === s.session_id}
          onclick={() => selectSession(s.session_id)}
        >
          <div class="sess-top">
            <span class="sess-name">{s.customer_name}</span>
            <span class="chip {statusBadgeClass(s.status)}">{friendlyStatus(s.status)}</span>
          </div>
          <div class="sess-sub mono">
            <span>{s.account_id}</span>
            {#if s.pending_overrides_count > 0}
              <span class="ov-count">{s.pending_overrides_count} override</span>
            {/if}
          </div>
          {#if s.last_message_preview}
            <div class="sess-prev">{s.last_message_preview}</div>
          {/if}
        </button>
      {/each}
    </div>
  </aside>

  <!-- CENTER PANE: Live Conversation + Model Visibility + Maker-Checker -->
  <section class="panel pane chat-pane">
    {#if activeSession}
      <div class="chat-head">
        <div>
          <div class="ch-title">
            <h3>{activeSession.customer_name}</h3>
            <span class="mono chip chip-muted">{activeSession.account_id}</span>
            <span class="chip {statusBadgeClass(activeSession.status)}">
              {friendlyStatus(activeSession.status)}
            </span>
          </div>
          <div class="ch-sub mono">
            Session {activeSession.session_id}
            {#if activeSession.assigned_officer}· Officer: {activeSession.assigned_officer}{/if}
          </div>
        </div>

        <div class="ch-actions">
          <button
            type="button"
            class="btn btn-sm {activeSession.status === 'STAFF_TAKEOVER' ? 'btn-emerald' : 'btn-primary'}"
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
            class="btn btn-sm {piiRevealed ? 'btn-amber' : ''}"
            onclick={toggleRevealPii}
            title="Audit-logged DPDP unmasking"
          >
            {#if piiRevealed}<EyeOff size={13} /> Hide PII{:else}<Eye size={13} /> Reveal PII{/if}
          </button>

          <button
            type="button"
            class="btn btn-sm"
            onclick={requestPurgePii}
            title="DPDP Right-to-Erasure: permanently wipe unmasked copies"
          >
            <Trash2 size={13} /> Purge PII
          </button>

          <button
            type="button"
            class="btn btn-sm {activeSession.status === 'QUARANTINED' ? 'btn-emerald' : 'btn-crimson'}"
            onclick={toggleQuarantine}
          >
            {#if activeSession.status === 'QUARANTINED'}
              <Unlock size={13} /> Unquarantine
            {:else}
              <Lock size={13} /> Quarantine
            {/if}
          </button>
        </div>
      </div>

      <!-- Pending Maker-Checker Approvals Banner -->
      {#each overrides.filter((o) => o.status === 'PENDING') as ov (ov.override_id)}
        <div class="override-card" transition:fly={{ y: -8, duration: 180 }}>
          <div class="ov-head">
            <span class="ov-title">
              <ShieldAlert size={15} color="#f59e0b" />
              Maker-Checker Staff Override Required: <span class="mono">{ov.tool_name}()</span>
            </span>
            <span class="chip chip-amber">{ov.violation_code}</span>
          </div>
          <div class="ov-reason">{ov.reason}</div>
          <div class="ov-controls">
            {#if ov.tool_args?.amount !== undefined}
              <label class="ov-field">
                <span>Refund Amount (₹)</span>
                <input type="number" class="input mono" bind:value={ovAmount[ov.override_id]} />
              </label>
            {/if}
            <label class="ov-field grow">
              <span>Officer Justification</span>
              <input
                type="text"
                class="input"
                placeholder="Verified merchant dispute receipt..."
                bind:value={ovNotes[ov.override_id]}
              />
            </label>
            <div class="ov-btns">
              <button
                type="button"
                class="btn btn-sm btn-emerald"
                onclick={() => confirmOverrideDecision(ov, 'APPROVE')}
              >
                <CheckCircle2 size={13} /> Approve & Execute
              </button>
              {#if ov.tool_args?.amount !== undefined}
                <button
                  type="button"
                  class="btn btn-sm btn-amber"
                  onclick={() => confirmOverrideDecision(ov, 'MODIFY_APPROVE')}
                >
                  Modify & Approve
                </button>
              {/if}
              <button
                type="button"
                class="btn btn-sm btn-crimson"
                onclick={() => confirmOverrideDecision(ov, 'REJECT')}
              >
                <XCircle size={13} /> Reject
              </button>
            </div>
          </div>
        </div>
      {/each}

      <!-- Messages Stream -->
      <div class="messages-stream" bind:this={chatStreamEl}>
        {#each messages as m (m.msg_id)}
          <div class="msg-row {m.role}" in:fly={{ y: 6, duration: 150 }}>
            <div class="msg-meta">
              <span class="role-lbl">
                {#if m.role === 'customer'}Customer
                {:else if m.role === 'assistant'}MoneyMax FinBot (AI)
                {:else if m.role === 'staff'}Staff Officer
                {:else}System Audit{/if}
              </span>
              {#if m.pii_counts && Object.keys(m.pii_counts).length > 0}
                <span class="chip chip-amber">
                  PII Masked: {Object.entries(m.pii_counts).map(([k, v]) => `${v} ${k}`).join(', ')}
                </span>
              {/if}
              <span class="mono muted time">{new Date(m.created_at).toLocaleTimeString()}</span>
            </div>

            <div class="msg-bubble">
              {piiRevealed && m.content_unmasked ? m.content_unmasked : m.content_sanitized}
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

      <!-- Dual Composer: Staff Override Reply OR Simulate Customer Message -->
      <form class="composer" onsubmit={sendMessage}>
        <div class="mode-switch">
          <button
            type="button"
            class="m-btn"
            class:active={composerMode === 'customer'}
            onclick={() => (composerMode = 'customer')}
          >
            Simulate Customer
          </button>
          <button
            type="button"
            class="m-btn staff"
            class:active={composerMode === 'staff'}
            onclick={() => (composerMode = 'staff')}
          >
            Send as Staff ({appState.officer})
          </button>

          {#if composerMode === 'customer'}
            <div class="sample-pills">
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
        <div class="comp-row">
          <input
            type="text"
            class="input"
            placeholder={composerMode === 'staff'
              ? 'Type manual staff override reply to customer...'
              : 'Type customer message to test AI + AegisMesh live...'}
            bind:value={composerText}
            disabled={sending}
          />
          <button type="submit" class="btn btn-primary" disabled={sending || !composerText.trim()}>
            <Send size={14} /> {sending ? 'Running...' : 'Send'}
          </button>
        </div>
      </form>
    {:else}
      <div class="empty-center">Select a customer session on the left to inspect chats & overrides.</div>
    {/if}
  </section>

  <!-- RIGHT PANE: Customer 360 + Manual Staff Actions -->
  <aside class="panel pane customer-pane">
    {#if customer}
      <div class="pane-head">
        <h3><Wallet size={15} /> Customer 360</h3>
        <span class="chip {customer.is_frozen ? 'chip-crimson' : 'chip-emerald'}">
          {customer.is_frozen ? 'FROZEN' : 'ACTIVE'}
        </span>
      </div>

      <div class="c-card panel-elevated">
        <div class="c-name">{customer.full_name}</div>
        <div class="mono muted c-id">{customer.account_id} · {customer.upi_vpa}</div>
        <div class="c-stats">
          <div>
            <span class="lbl">Balance</span>
            <div class="val mono cyan">₹{customer.balance.toLocaleString('en-IN', { minimumFractionDigits: 2 })}</div>
          </div>
          <div>
            <span class="lbl">Credit Limit</span>
            <div class="val mono">₹{customer.credit_limit.toLocaleString('en-IN')}</div>
          </div>
          <div>
            <span class="lbl">KYC Status</span>
            <div>
              <span class="chip {customer.kyc_status === 'VERIFIED' ? 'chip-emerald' : 'chip-amber'}">
                {customer.kyc_status}
              </span>
            </div>
          </div>
          <div>
            <span class="lbl">CIBIL / Risk</span>
            <div class="mono">{customer.cibil_score} · {customer.risk_tier}</div>
          </div>
        </div>
      </div>

      <!-- Direct Staff Ledger Actions -->
      <div class="manual-box panel-elevated">
        <div class="lbl">Manual Staff Ledger Override</div>
        <div class="man-tabs">
          <button type="button" class:active={manualTab === 'refund'} onclick={() => (manualTab = 'refund')}>Refund</button>
          <button type="button" class:active={manualTab === 'kyc'} onclick={() => (manualTab = 'kyc')}>KYC</button>
          <button type="button" class:active={manualTab === 'credit'} onclick={() => (manualTab = 'credit')}>Credit</button>
          <button type="button" class:active={manualTab === 'freeze'} onclick={() => (manualTab = 'freeze')}>Hold</button>
        </div>

        {#if manualTab === 'refund'}
          <input type="number" class="input mono" placeholder="Amount ₹" bind:value={manAmount} />
        {:else if manualTab === 'kyc'}
          <select class="select" bind:value={manKyc}>
            <option value="VERIFIED">VERIFIED</option>
            <option value="PENDING_REVIEW">PENDING_REVIEW</option>
            <option value="FLAGGED">FLAGGED</option>
            <option value="REJECTED">REJECTED</option>
          </select>
        {:else if manualTab === 'credit'}
          <input type="number" class="input mono" placeholder="New limit ₹" bind:value={manCredit} />
        {:else}
          <div class="muted" style="font-size: 12px;">
            Account is currently <b>{customer.is_frozen ? 'FROZEN' : 'ACTIVE'}</b>.
          </div>
        {/if}

        <input type="text" class="input" placeholder="Audit justification..." bind:value={manReason} />
        <button type="button" class="btn btn-amber btn-sm" onclick={executeManualStaffAction}>
          {manualTab === 'freeze'
            ? customer.is_frozen
              ? 'Unfreeze Account'
              : 'Freeze Account'
            : `Execute Staff ${manualTab.toUpperCase()}`}
        </button>
      </div>

      <!-- Open Disputes & Recent Transactions -->
      <div class="mini-section">
        <div class="lbl">Open Disputes ({disputes.length})</div>
        {#each disputes.slice(0, 3) as d}
          <div class="mini-row">
            <div class="mono" style="font-size: 11px;">{d.dispute_id} · ₹{d.claimed_amount}</div>
            <span class="chip {d.status === 'OPEN' ? 'chip-amber' : 'chip-emerald'}">{d.status}</span>
          </div>
        {/each}
      </div>

      <div class="mini-section">
        <div class="lbl">Recent Ledger Transactions</div>
        <div class="txn-scroll">
          {#each transactions.slice(0, 6) as t}
            <div class="mini-row">
              <div>
                <div class="mono" style="font-size: 11px;">{t.txn_id}</div>
                <div class="muted" style="font-size: 10.5px;">{t.counterparty}</div>
              </div>
              <div class="mono" style="text-align: right;">
                <div class={t.direction === 'CREDIT' ? 'emerald' : ''}>
                  {t.direction === 'CREDIT' ? '+' : '-'}₹{Number(t.amount).toLocaleString('en-IN')}
                </div>
                <div class="muted" style="font-size: 10px;">{t.status}</div>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <div class="empty-center">Customer ledger details appear here.</div>
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
    grid-template-columns: 270px minmax(0, 1fr) 310px;
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
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .pane-head h3 {
    font-size: 13.5px;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .picker {
    margin: 8px 10px 0;
    padding: 8px;
  }
  .picker-title {
    font-size: 11px;
    color: var(--text-1);
    margin-bottom: 6px;
  }
  .picker-list {
    max-height: 180px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .picker-item {
    display: flex;
    justify-content: space-between;
    padding: 5px 7px;
    font-size: 11.5px;
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-0);
    cursor: pointer;
  }
  .picker-item:hover {
    border-color: var(--cyan);
  }
  .search-box {
    position: relative;
    padding: 10px 10px 6px;
  }
  .search-box :global(.s-icon) {
    position: absolute;
    left: 20px;
    top: 20px;
    color: var(--text-2);
  }
  .s-input {
    padding-left: 28px;
    font-size: 12px;
  }
  .status-tabs {
    display: flex;
    gap: 4px;
    padding: 4px 10px 8px;
    flex-wrap: wrap;
  }
  .s-tab {
    padding: 3px 7px;
    font-size: 10.5px;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
  }
  .s-tab.active {
    background: var(--cyan-dim);
    border-color: rgba(34, 211, 238, 0.4);
    color: var(--cyan);
  }
  .session-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .sess-card {
    text-align: left;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 9px 10px;
    color: var(--text-0);
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 4px;
    transition: border-color 140ms ease, background 140ms ease;
  }
  .sess-card:hover {
    border-color: var(--border-bright);
  }
  .sess-card.selected {
    border-color: var(--cyan);
    background: rgba(34, 211, 238, 0.06);
  }
  .sess-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 6px;
  }
  .sess-name {
    font-weight: 600;
    font-size: 12.5px;
  }
  .sess-sub {
    font-size: 11px;
    color: var(--text-2);
    display: flex;
    justify-content: space-between;
  }
  .ov-count {
    color: var(--amber);
    font-weight: 600;
  }
  .sess-prev {
    font-size: 11.5px;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Center Chat */
  .chat-head {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .ch-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ch-title h3 {
    font-size: 14.5px;
    font-weight: 600;
  }
  .ch-sub {
    font-size: 11px;
    color: var(--text-2);
    margin-top: 2px;
  }
  .ch-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .override-card {
    margin: 10px 14px 0;
    padding: 12px 14px;
    background: linear-gradient(180deg, rgba(245, 158, 11, 0.12), rgba(17, 25, 39, 0.95));
    border: 1px solid rgba(245, 158, 11, 0.45);
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .ov-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .ov-title {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    font-size: 12.5px;
  }
  .ov-reason {
    font-size: 12px;
    color: var(--text-1);
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
    gap: 3px;
    font-size: 10.5px;
    color: var(--text-1);
  }
  .ov-field.grow {
    flex: 1;
    min-width: 170px;
  }
  .ov-btns {
    display: flex;
    gap: 6px;
  }

  .messages-stream {
    flex: 1;
    overflow-y: auto;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .msg-row {
    display: flex;
    flex-direction: column;
    max-width: 86%;
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
    max-width: 95%;
  }
  .msg-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
    font-size: 11px;
  }
  .role-lbl {
    font-weight: 600;
    color: var(--text-1);
  }
  .time {
    font-size: 10px;
  }
  .msg-bubble {
    padding: 10px 13px;
    border-radius: 10px;
    font-size: 13px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    white-space: pre-wrap;
  }
  .msg-row.assistant .msg-bubble {
    background: rgba(34, 211, 238, 0.08);
    border-color: rgba(34, 211, 238, 0.24);
  }
  .msg-row.staff .msg-bubble {
    background: rgba(167, 139, 250, 0.12);
    border-color: rgba(167, 139, 250, 0.35);
  }
  .msg-row.system .msg-bubble {
    background: rgba(148, 163, 184, 0.06);
    font-size: 11.5px;
    color: var(--text-1);
    text-align: center;
  }

  .composer {
    border-top: 1px solid var(--border);
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--bg-1);
  }
  .mode-switch {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .sample-pills {
    margin-left: auto;
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
  }
  .sample-pill {
    padding: 2px 8px;
    font-size: 10.5px;
    border-radius: 999px;
    border: 1px dashed rgba(34, 211, 238, 0.32);
    background: rgba(34, 211, 238, 0.05);
    color: var(--cyan);
    cursor: pointer;
    transition: background 140ms ease, border-color 140ms ease;
  }
  .sample-pill:hover {
    background: rgba(34, 211, 238, 0.14);
    border-color: var(--cyan);
  }
  .m-btn {
    padding: 3px 10px;
    font-size: 11px;
    font-weight: 600;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
  }
  .m-btn.active {
    background: var(--cyan-dim);
    border-color: var(--cyan);
    color: var(--cyan);
  }
  .m-btn.staff.active {
    background: var(--violet-dim);
    border-color: var(--violet);
    color: var(--violet);
  }
  .comp-row {
    display: flex;
    gap: 8px;
  }

  /* Customer 360 */
  .customer-pane {
    overflow-y: auto;
  }
  .c-card,
  .manual-box {
    margin: 10px 12px 0;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .c-name {
    font-size: 14px;
    font-weight: 600;
  }
  .c-id {
    font-size: 11px;
  }
  .c-stats {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-top: 4px;
  }
  .lbl {
    font-size: 10.5px;
    text-transform: uppercase;
    color: var(--text-2);
    letter-spacing: 0.04em;
  }
  .val {
    font-size: 13.5px;
    font-weight: 600;
  }
  .cyan { color: var(--cyan); }
  .emerald { color: var(--emerald); }
  .muted { color: var(--text-2); }

  .man-tabs {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
  }
  .man-tabs button {
    padding: 4px;
    font-size: 11px;
    font-weight: 600;
    border-radius: 4px;
    border: 1px solid var(--border);
    background: var(--bg-1);
    color: var(--text-1);
    cursor: pointer;
  }
  .man-tabs button.active {
    background: var(--amber-dim);
    border-color: rgba(245, 158, 11, 0.45);
    color: var(--amber);
  }

  .mini-section {
    margin: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .mini-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 8px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: 6px;
    font-size: 11.5px;
  }
  .txn-scroll {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .empty-center {
    margin: auto;
    color: var(--text-2);
    padding: 24px;
    text-align: center;
  }

  @media (max-width: 1180px) {
    .ops-grid {
      grid-template-columns: 240px minmax(0, 1fr);
    }
    .customer-pane {
      display: none;
    }
  }
</style>
