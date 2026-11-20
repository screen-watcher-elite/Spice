<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api, getAdminKey, setAdminKey } from './lib/api';
  import { appState, type NavTab } from './lib/stores/appState.svelte';
  import ConfirmModal from './lib/components/ConfirmModal.svelte';
  import OverviewView from './views/OverviewView.svelte';
  import ChatsOverrideView from './views/ChatsOverrideView.svelte';
  import SandboxView from './views/SandboxView.svelte';
  import SentinelView from './views/SentinelView.svelte';
  import SettingsView from './views/SettingsView.svelte';
  import { fly, fade } from 'svelte/transition';
  import {
    Shield,
    ShieldOff,
    Activity,
    MessageSquareCode,
    FlaskConical,
    Radar,
    Settings,
    KeyRound,
    CheckCircle2,
    AlertTriangle,
    ShieldX,
    Info,
    HelpCircle,
    X,
  } from 'lucide-svelte';

  let showAdminPopover = $state(false);
  let adminInput = $state(getAdminKey());
  let adminError = $state('');
  let verifyingKey = $state(false);
  let showBypassConfirm = $state(false);
  let showHelp = $state(false);

  const navItems: Array<{ id: NavTab; label: string; key: string }> = [
    { id: 'overview', label: 'Overview', key: '1' },
    { id: 'chats', label: 'Customer Chats', key: '2' },
    { id: 'sandbox', label: 'Attack Sandbox', key: '3' },
    { id: 'sentinel', label: 'AI Sentinel', key: '4' },
    { id: 'settings', label: 'Settings', key: '5' },
  ];

  async function executeShieldToggle(next: boolean) {
    try {
      const res = await api.toggleShield(next, appState.officer);
      appState.pushToast(next ? 'success' : 'warn', res.message);
      await appState.refreshAll();
    } catch (e: unknown) {
      appState.pushToast('error', 'Admin Key Required', (e as Error).message);
      showAdminPopover = true;
    }
  }

  function handleShieldClick() {
    if (!appState.shieldStatus) return;
    const next = !appState.shieldStatus.shield_active;
    if (!next) {
      showBypassConfirm = true;
    } else {
      executeShieldToggle(true);
    }
  }

  async function applyAdminKey(e: Event) {
    e.preventDefault();
    adminError = '';
    verifyingKey = true;
    setAdminKey(adminInput);
    try {
      const res = await api.verifyAdminKey();
      appState.adminVerified = res.valid;
      showAdminPopover = false;
      appState.pushToast(
        'success',
        res.admin_key_required ? 'Admin Key Verified' : 'Gateway Unlocked (No Key Configured)',
      );
    } catch (err: unknown) {
      setAdminKey('');
      appState.adminVerified = false;
      adminError = (err as Error).message;
    } finally {
      verifyingKey = false;
    }
  }

  function lockAdminKey() {
    adminInput = '';
    adminError = '';
    setAdminKey('');
    appState.adminVerified = false;
    showAdminPopover = false;
    appState.pushToast('info', 'Admin Session Locked');
  }

  function handleGlobalKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      showAdminPopover = false;
      showHelp = false;
      return;
    }
    const target = e.target as HTMLElement | null;
    if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return;

    if (e.key === '?') {
      showHelp = !showHelp;
      return;
    }
    const match = navItems.find((n) => n.key === e.key);
    if (match) {
      appState.activeTab = match.id;
    }
  }

  onMount(async () => {
    await appState.refreshAll();
    if (getAdminKey()) {
      api
        .verifyAdminKey()
        .then((r) => (appState.adminVerified = r.valid))
        .catch(() => {
          setAdminKey('');
          appState.adminVerified = false;
        });
    }
    appState.connectSse();
  });

  onDestroy(() => {
    appState.disconnectSse();
  });
</script>

<svelte:window onkeydown={handleGlobalKey} />

<div class="shell">
  <!-- Slim Left Rail -->
  <aside class="rail">
    <div class="brand">
      <div class="logo-box">
        <Shield size={18} color="#22d3ee" />
      </div>
      <div>
        <div class="b-title">AegisMesh</div>
        <div class="b-sub mono">RUST · v2.0</div>
      </div>
    </div>

    <nav class="nav">
      {#each navItems as item}
        <button
          type="button"
          class="nav-item"
          class:active={appState.activeTab === item.id}
          onclick={() => (appState.activeTab = item.id)}
        >
          {#if item.id === 'overview'}<Activity size={16} />
          {:else if item.id === 'chats'}<MessageSquareCode size={16} />
          {:else if item.id === 'sandbox'}<FlaskConical size={16} />
          {:else if item.id === 'sentinel'}<Radar size={16} />
          {:else}<Settings size={16} />{/if}
          <span>{item.label}</span>
          {#if item.id === 'chats' && appState.pendingOverridesCount > 0}
            <span class="badge">{appState.pendingOverridesCount}</span>
          {/if}
        </button>
      {/each}
    </nav>

    <div class="rail-footer">
      <div class="status-row">
        <span class="pulse-beacon" style="color: {appState.sseConnected ? '#10b981' : '#f59e0b'};"></span>
        <span>{appState.sseConnected ? 'Live Telemetry' : 'Reconnecting...'}</span>
      </div>
      <div class="muted mono" style="font-size: 10.5px;">
        AI: {appState.shieldStatus?.gemini_configured ? 'Gemini Live' : 'Offline Simulator'}
      </div>
    </div>
  </aside>

  <!-- Main Workspace -->
  <div class="main-col">
    <header class="topbar">
      <div class="tb-left">
        <span class="view-title">
          {navItems.find((n) => n.id === appState.activeTab)?.label}
        </span>
        <span class="chip chip-muted mono">
          {appState.shieldStatus?.policy_preset || 'RBI_DPDP_FINTECH_V1'}
        </span>
      </div>

      <div class="tb-right">
        <button
          type="button"
          class="btn btn-sm"
          onclick={() => (showHelp = true)}
          title="Quick Guide & Shortcuts (?)"
          aria-label="Quick Guide"
        >
          <HelpCircle size={14} />
        </button>

        <label class="officer-pick">
          <span class="muted">Officer</span>
          <select class="select sel-sm" bind:value={appState.officer}>
            <option value="Officer Ashutosh">Officer Ashutosh</option>
            <option value="Officer Rao">Officer Rao</option>
            <option value="Officer Mehta">Officer Mehta</option>
          </select>
        </label>

        <div class="admin-wrap">
          <button
            type="button"
            class="btn btn-sm {appState.adminVerified ? 'btn-emerald' : ''}"
            onclick={() => {
              adminError = '';
              showAdminPopover = !showAdminPopover;
            }}
            title="Admin Key for protected staff actions"
          >
            <KeyRound size={13} />
            <span>{appState.adminVerified ? 'Admin Verified' : 'Admin Key'}</span>
          </button>

          {#if showAdminPopover}
            <form class="admin-pop panel" onsubmit={applyAdminKey} transition:fly={{ y: -6, duration: 140 }}>
              <div class="muted" style="font-size: 11px;">
                {appState.shieldStatus?.admin_key_required
                  ? 'Enter Admin Key to unlock staff controls:'
                  : 'No Admin Key configured (open local mode):'}
              </div>
              <input
                type="password"
                class="input"
                placeholder="AEGIS_ADMIN_KEY"
                bind:value={adminInput}
                disabled={verifyingKey}
              />
              {#if adminError}
                <div class="err-msg">{adminError}</div>
              {/if}
              <div class="pop-btns">
                <button type="submit" class="btn btn-sm btn-primary" disabled={verifyingKey}>
                  {verifyingKey ? 'Checking...' : 'Verify'}
                </button>
                {#if appState.adminVerified}
                  <button type="button" class="btn btn-sm" onclick={lockAdminKey}>Lock</button>
                {/if}
              </div>
            </form>
          {/if}
        </div>

        <button
          type="button"
          class="shield-btn"
          class:active={appState.shieldStatus?.shield_active ?? true}
          onclick={handleShieldClick}
        >
          {#if appState.shieldStatus?.shield_active ?? true}
            <Shield size={14} />
            <span>SHIELD ACTIVE</span>
          {:else}
            <ShieldOff size={14} />
            <span>SHIELD BYPASSED</span>
          {/if}
        </button>
      </div>
    </header>

    <main class="stage">
      {#if appState.activeTab === 'overview'}
        <OverviewView />
      {:else if appState.activeTab === 'chats'}
        <ChatsOverrideView />
      {:else if appState.activeTab === 'sandbox'}
        <SandboxView />
      {:else if appState.activeTab === 'sentinel'}
        <SentinelView />
      {:else}
        <SettingsView />
      {/if}
    </main>
  </div>

  <!-- Toast Stack -->
  <div class="toasts">
    {#each appState.toasts as t (t.id)}
      <div class="toast panel {t.kind}" transition:fly={{ x: 40, duration: 180 }}>
        <div class="t-title">
          {#if t.kind === 'success'}
            <CheckCircle2 size={14} color="#10b981" />
          {:else if t.kind === 'warn'}
            <AlertTriangle size={14} color="#f59e0b" />
          {:else if t.kind === 'error'}
            <ShieldX size={14} color="#f43f5e" />
          {:else}
            <Info size={14} color="#22d3ee" />
          {/if}
          <span>{t.title}</span>
        </div>
        {#if t.detail}
          <div class="t-det">{t.detail}</div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<!-- Shield Bypass Confirmation Modal -->
<ConfirmModal
  open={showBypassConfirm}
  title="Bypass 6-Layer Security Shield?"
  message="Disabling the shield turns off inline prompt injection filtering, DPDP PII masking, and RBI circuit-breaker checks so you can compare unprotected agent behavior. Layer 6 Sentinel will still watch asynchronously."
  confirmLabel="Bypass Shield (Demo Mode)"
  tone="crimson"
  onCancel={() => (showBypassConfirm = false)}
  onConfirm={() => {
    showBypassConfirm = false;
    executeShieldToggle(false);
  }}
/>

<!-- Quick Help Modal -->
{#if showHelp}
  <div class="help-back" transition:fade={{ duration: 130 }}>
    <button
      type="button"
      class="help-dismiss"
      aria-label="Close help"
      onclick={() => (showHelp = false)}
    ></button>
    <div class="help-card panel" role="dialog" aria-modal="true" aria-label="AegisMesh Quick Guide">
      <div class="h-head">
        <h3>AegisMesh Quick Guide</h3>
        <button type="button" class="btn btn-sm" onclick={() => (showHelp = false)}>
          <X size={14} />
        </button>
      </div>
      <div class="h-grid">
        <div><b>1 · Overview</b> — Live latency percentiles, 6-layer pipeline timings, and real-time event inspector.</div>
        <div><b>2 · Customer Chats</b> — Inspect live customer chats, review AI tool calls, take over manually, or approve/reject held ₹5,000+ refunds.</div>
        <div><b>3 · Attack Sandbox</b> — One-click adversarial simulations comparing raw input vs what the AI model actually receives.</div>
        <div><b>4 · AI Sentinel</b> — Asynchronous watcher catching multi-turn drift or reverting unauthorized payouts + CSV/JSON exports.</div>
        <div><b>5 · Settings</b> — Adjust RBI limits, DPDP PII rules, Gemini API key, and Admin Key lock live without restarting.</div>
      </div>
      <div class="muted mono" style="font-size: 11px;">
        Shortcuts: Keys 1–5 switch views · ? toggles help · Esc closes modals
      </div>
    </div>
  </div>
{/if}

<style>
  .shell {
    display: grid;
    grid-template-columns: 218px minmax(0, 1fr);
    height: 100vh;
    background: radial-gradient(circle at top right, rgba(34, 211, 238, 0.05), transparent 42%),
      var(--bg-0);
  }
  .rail {
    background: var(--bg-1);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    padding: 14px 10px;
    gap: 18px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 6px;
  }
  .logo-box {
    width: 34px;
    height: 34px;
    border-radius: 9px;
    background: var(--cyan-dim);
    border: 1px solid rgba(34, 211, 238, 0.35);
    display: grid;
    place-items: center;
  }
  .b-title {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .b-sub {
    font-size: 10px;
    color: var(--cyan);
  }
  .nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 11px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-1);
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    cursor: pointer;
    transition: all 140ms var(--ease-out);
    text-align: left;
  }
  .nav-item:hover {
    background: var(--bg-2);
    color: var(--text-0);
  }
  .nav-item.active {
    background: var(--cyan-dim);
    border-color: rgba(34, 211, 238, 0.28);
    color: var(--cyan);
  }
  .badge {
    margin-left: auto;
    padding: 1px 6px;
    border-radius: 999px;
    font-size: 10.5px;
    font-weight: 700;
    background: var(--amber);
    color: #060a11;
  }
  .rail-footer {
    margin-top: auto;
    padding: 10px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11.5px;
    font-weight: 500;
  }

  .main-col {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .topbar {
    height: 54px;
    padding: 0 18px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(11, 17, 28, 0.85);
    backdrop-filter: blur(8px);
    z-index: 15;
  }
  .tb-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .view-title {
    font-size: 15px;
    font-weight: 600;
  }
  .tb-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .officer-pick {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }
  .sel-sm {
    padding: 4px 8px;
    font-size: 12px;
    width: auto;
  }
  .admin-wrap {
    position: relative;
  }
  .admin-pop {
    position: absolute;
    right: 0;
    top: calc(100% + 8px);
    width: 260px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 30;
    box-shadow: 0 14px 30px rgba(0, 0, 0, 0.45);
  }
  .err-msg {
    font-size: 11.5px;
    color: var(--crimson);
  }
  .pop-btns {
    display: flex;
    gap: 6px;
  }
  .shield-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 6px 13px;
    border-radius: 999px;
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    border: 1px solid rgba(244, 63, 94, 0.5);
    background: var(--crimson-dim);
    color: var(--crimson);
    cursor: pointer;
    transition: all 160ms var(--ease-out);
  }
  .shield-btn.active {
    border-color: rgba(16, 185, 129, 0.5);
    background: var(--emerald-dim);
    color: var(--emerald);
  }

  .stage {
    flex: 1;
    min-height: 0;
    padding: 14px 16px;
    overflow: hidden;
  }
  .muted { color: var(--text-2); }

  .toasts {
    position: fixed;
    bottom: 18px;
    right: 18px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 320px;
    pointer-events: none;
  }
  .toast {
    padding: 10px 13px;
    pointer-events: auto;
    border-left: 3px solid var(--cyan);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.45);
  }
  .toast.success { border-left-color: var(--emerald); }
  .toast.warn { border-left-color: var(--amber); }
  .toast.error { border-left-color: var(--crimson); }
  .t-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    font-weight: 600;
  }
  .t-det {
    font-size: 11.5px;
    color: var(--text-1);
    margin-top: 2px;
  }

  .help-back {
    position: fixed;
    inset: 0;
    z-index: 65;
    background: rgba(4, 7, 12, 0.72);
    backdrop-filter: blur(4px);
    display: grid;
    place-items: center;
  }
  .help-dismiss {
    position: fixed;
    inset: 0;
    border: none;
    background: transparent;
    cursor: pointer;
  }
  .help-card {
    position: relative;
    z-index: 1;
    width: min(520px, 92vw);
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .h-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .h-grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-1);
  }
  .h-grid b {
    color: var(--text-0);
  }

  @media (max-width: 820px) {
    .shell {
      grid-template-columns: 64px minmax(0, 1fr);
    }
    .brand > div:last-child,
    .nav-item span:not(.badge),
    .rail-footer {
      display: none;
    }
  }
</style>
