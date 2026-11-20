<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api, getAdminKey, setAdminKey } from './lib/api';
  import { appState, type NavTab } from './lib/stores/appState.svelte';
  import ConfirmModal from './lib/components/ConfirmModal.svelte';
  import CommandPalette, { type PaletteCommand } from './lib/components/CommandPalette.svelte';
  import Logo from './lib/components/Logo.svelte';
  import OverviewView from './views/OverviewView.svelte';
  import ChatsOverrideView from './views/ChatsOverrideView.svelte';
  import SandboxView from './views/SandboxView.svelte';
  import SentinelView from './views/SentinelView.svelte';
  import SettingsView from './views/SettingsView.svelte';
  import { fly, fade, scale } from 'svelte/transition';
  import { initials } from './lib/ui';
  import {
    Shield,
    ShieldOff,
    LayoutGrid,
    MessagesSquare,
    FlaskConical,
    Radar,
    SlidersHorizontal,
    KeyRound,
    LockKeyhole,
    Check,
    TriangleAlert,
    ShieldX,
    Info,
    Search,
    Keyboard,
    Download,
    Sun,
    Moon,
    Sparkles,
    X,
  } from 'lucide-svelte';

  let adminInput = $state(getAdminKey());
  let adminError = $state('');
  let verifyingKey = $state(false);
  let showBypassConfirm = $state(false);
  let showHelp = $state(false);
  let paletteOpen = $state(false);

  const navItems: Array<{ id: NavTab; label: string; key: string }> = [
    { id: 'overview', label: 'Overview', key: '1' },
    { id: 'chats', label: 'Chats', key: '2' },
    { id: 'sandbox', label: 'Sandbox', key: '3' },
    { id: 'sentinel', label: 'Sentinel', key: '4' },
    { id: 'settings', label: 'Settings', key: '5' },
  ];

  const armed = $derived(appState.shieldStatus?.shield_active ?? true);

  /* ---------- sliding nav indicator ---------- */
  const navBtns: HTMLButtonElement[] = [];
  let ind = $state({ x: 0, w: 0, ready: false });

  function measure() {
    const i = navItems.findIndex((n) => n.id === appState.activeTab);
    const el = navBtns[i];
    if (el) ind = { x: el.offsetLeft, w: el.offsetWidth, ready: true };
  }

  $effect(() => {
    void appState.activeTab;
    void appState.pendingOverridesCount;
    void appState.demoMode;
    requestAnimationFrame(measure);
  });

  /* ---------- shield ---------- */
  async function executeShieldToggle(next: boolean) {
    try {
      const res = await api.toggleShield(next, appState.officer);
      appState.pushToast(next ? 'success' : 'warn', res.message);
      await appState.refreshAll();
    } catch (e: unknown) {
      appState.requestAdminUnlock((e as Error).message);
    }
  }

  function handleShieldClick() {
    if (!appState.shieldStatus) return;
    const next = !appState.shieldStatus.shield_active;
    if (!next) showBypassConfirm = true;
    else executeShieldToggle(true);
  }

  /* ---------- admin key ---------- */
  async function applyAdminKey(e: Event) {
    e.preventDefault();
    adminError = '';
    verifyingKey = true;
    setAdminKey(adminInput);
    try {
      const res = await api.verifyAdminKey();
      appState.adminVerified = res.valid;
      appState.showAdminPopover = false;
      appState.pushToast('success', res.admin_key_required ? 'Admin key verified' : 'Unlocked — no key configured');
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
    appState.showAdminPopover = false;
    appState.pushToast('info', 'Admin session locked');
  }

  async function exportAudit(fmt: 'json' | 'csv') {
    try {
      await api.downloadAuditReport(fmt);
      appState.pushToast('success', `Exported ${fmt.toUpperCase()} audit report`);
    } catch (e: unknown) {
      appState.requestAdminUnlock((e as Error).message);
    }
  }

  /* ---------- command palette ---------- */
  const navIcons: Record<NavTab, unknown> = {
    overview: LayoutGrid,
    chats: MessagesSquare,
    sandbox: FlaskConical,
    sentinel: Radar,
    settings: SlidersHorizontal,
  };

  const commands = $derived<PaletteCommand[]>([
    ...navItems.map((n) => ({
      id: `nav-${n.id}`,
      label: `Go to ${n.label}`,
      group: 'Navigate',
      keys: n.key,
      icon: navIcons[n.id],
      run: () => (appState.activeTab = n.id),
    })),
    {
      id: 'shield',
      label: armed ? 'Bypass security shield' : 'Re-arm security shield',
      group: 'Perimeter',
      hint: armed ? 'demo mode' : 'restore 6 layers',
      icon: armed ? ShieldOff : Shield,
      run: handleShieldClick,
    },
    {
      id: 'admin',
      label: appState.adminVerified ? 'Lock admin session' : 'Enter admin key',
      group: 'Perimeter',
      icon: appState.adminVerified ? LockKeyhole : KeyRound,
      run: () => (appState.adminVerified ? lockAdminKey() : (appState.showAdminPopover = true)),
    },
    { id: 'exp-json', label: 'Export audit report', group: 'Compliance', hint: 'JSON', icon: Download, run: () => exportAudit('json') },
    { id: 'exp-csv', label: 'Export audit report', group: 'Compliance', hint: 'CSV', icon: Download, run: () => exportAudit('csv') },
    {
      id: 'theme',
      label: appState.theme === 'dark' ? 'Switch to Light Mode' : 'Switch to Dark Mode',
      group: 'Appearance',
      keys: 'T',
      icon: appState.theme === 'dark' ? Sun : Moon,
      run: () => appState.toggleTheme(),
    },
    {
      id: 'demo',
      label: appState.demoMode ? 'Exit Demo Mode (Full Telemetry)' : 'Switch to Minimal Demo Mode',
      group: 'Appearance',
      keys: 'D',
      icon: Sparkles,
      run: () => appState.toggleDemoMode(),
    },
    { id: 'help', label: 'Keyboard shortcuts', group: 'Help', keys: '?', icon: Keyboard, run: () => (showHelp = true) },
  ]);

  /* ---------- keyboard ---------- */
  function handleGlobalKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      paletteOpen = !paletteOpen;
      return;
    }
    if (paletteOpen) return;
    if (e.key === 'Escape') {
      appState.showAdminPopover = false;
      showHelp = false;
      return;
    }
    const target = e.target as HTMLElement | null;
    if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return;
    if (e.metaKey || e.ctrlKey || e.altKey) return;

    if (e.key === '?') {
      showHelp = !showHelp;
      return;
    }
    if (e.key.toLowerCase() === 't') {
      appState.toggleTheme();
      return;
    }
    if (e.key.toLowerCase() === 'd') {
      appState.toggleDemoMode();
      return;
    }
    const match = navItems.find((n) => n.key === e.key);
    if (match) appState.activeTab = match.id;
  }

  onMount(async () => {
    document.fonts?.ready.then(measure);
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

  onDestroy(() => appState.disconnectSse());
</script>

<svelte:window onkeydown={handleGlobalKey} onresize={measure} />

<div class="ambient" class:breach={!armed} aria-hidden="true">
  <div class="aurora"></div>
  <div class="dots"></div>
  <div class="grain"></div>
</div>

<div class="shell" class:breach={!armed}>
  <header class="topbar">
    <div class="tb-left">
      <button type="button" class="brand" onclick={() => (appState.activeTab = 'overview')} aria-label="AegisMesh home">
        <Logo size={24} {armed} />
        <span class="wordmark">AegisMesh</span>
      </button>
      <span class="sep" aria-hidden="true"></span>
      <span class="policy mono" title="Active policy preset">
        {appState.shieldStatus?.policy_preset || 'RBI_DPDP_FINTECH_V1'}
      </span>
    </div>

    <nav class="nav" aria-label="Primary">
      <span
        class="indicator"
        class:ready={ind.ready}
        style="transform: translateX({ind.x}px); width: {ind.w}px;"
        aria-hidden="true"
      ></span>
      {#each navItems as item, i (item.id)}
        <button
          type="button"
          class="nav-item"
          class:active={appState.activeTab === item.id}
          bind:this={navBtns[i]}
          onclick={() => (appState.activeTab = item.id)}
          title="{item.label} · {item.key}"
        >
          {#if item.id === 'overview'}<LayoutGrid size={14} strokeWidth={1.8} />
          {:else if item.id === 'chats'}<MessagesSquare size={14} strokeWidth={1.8} />
          {:else if item.id === 'sandbox'}<FlaskConical size={14} strokeWidth={1.8} />
          {:else if item.id === 'sentinel'}<Radar size={14} strokeWidth={1.8} />
          {:else}<SlidersHorizontal size={14} strokeWidth={1.8} />{/if}
          <span class="nl">{item.label}</span>
          {#if item.id === 'chats' && appState.pendingOverridesCount > 0}
            <span class="badge mono" in:scale={{ duration: 260 }}>{appState.pendingOverridesCount}</span>
          {/if}
        </button>
      {/each}
    </nav>

    <div class="tb-right">
      <button type="button" class="cmdk" onclick={() => (paletteOpen = true)} aria-label="Open command palette">
        <Search size={13} strokeWidth={2} />
        <span class="ck-l">Search</span>
        <span class="kbd">Ctrl K</span>
      </button>

      <div
        class="live mono"
        title={appState.shieldStatus?.gemini_configured ? 'Gemini live · telemetry stream' : 'Offline simulator · telemetry stream'}
      >
        <span class="dot live" style="color: {appState.sseConnected ? 'var(--accent)' : 'var(--warn)'};"></span>
        <span>{appState.sseConnected ? 'LIVE' : 'SYNC'}</span>
        <span class="engine">{appState.shieldStatus?.gemini_configured ? 'GEMINI' : 'SIM'}</span>
      </div>

      <label class="officer" title="Acting officer">
        <span class="av">{initials(appState.officer.replace('Officer ', ''))}</span>
        <select bind:value={appState.officer} aria-label="Acting officer">
          <option value="Officer Ashutosh">Ashutosh</option>
          <option value="Officer Rao">Rao</option>
          <option value="Officer Mehta">Mehta</option>
        </select>
      </label>

      <button
        type="button"
        class="demo-btn"
        class:active={appState.demoMode}
        onclick={() => appState.toggleDemoMode()}
        title={appState.demoMode ? 'Exit Minimal Demo Mode (D)' : 'Enter Minimal Demo Mode (D)'}
        aria-pressed={appState.demoMode}
        aria-label="Toggle Minimal Demo Mode"
      >
        <Sparkles size={13} strokeWidth={2} />
        <span>Demo</span>
      </button>

      <button
        type="button"
        class="btn btn-icon btn-ghost theme-btn"
        onclick={() => appState.toggleTheme()}
        title={appState.theme === 'dark' ? 'Switch to Light Mode (T)' : 'Switch to Dark Mode (T)'}
        aria-label="Toggle color theme"
      >
        {#if appState.theme === 'dark'}
          <Sun size={15} strokeWidth={1.8} />
        {:else}
          <Moon size={15} strokeWidth={1.8} />
        {/if}
      </button>

      <div class="admin-wrap">
        <button
          type="button"
          class="btn btn-icon btn-ghost key-btn"
          class:verified={appState.adminVerified}
          onclick={() => {
            adminError = '';
            appState.showAdminPopover = !appState.showAdminPopover;
          }}
          title={appState.adminVerified ? 'Admin verified' : 'Admin key'}
          aria-label="Admin key"
        >
          {#if appState.adminVerified}<LockKeyhole size={15} strokeWidth={1.8} />{:else}<KeyRound size={15} strokeWidth={1.8} />{/if}
          {#if appState.adminVerified}<span class="kdot"></span>{/if}
        </button>

        {#if appState.showAdminPopover}
          <form class="admin-pop glass" onsubmit={applyAdminKey} transition:fly={{ y: -6, duration: 180 }}>
            <div class="ap-head">
              <span class="serif ap-title">Staff unlock</span>
              <span class="tag {appState.adminVerified ? 'ok' : ''} dotted">
                {appState.adminVerified ? 'verified' : 'locked'}
              </span>
            </div>
            <p class="ap-sub">
              {appState.shieldStatus?.admin_key_required
                ? 'Protected actions — shield bypass, PII reveal, exports — require the admin key.'
                : 'No admin key configured. Gateway is in open local mode.'}
            </p>
            <!-- svelte-ignore a11y_autofocus -->
            <input
              type="password"
              class="input mono"
              placeholder="AEGIS_ADMIN_KEY"
              bind:value={adminInput}
              disabled={verifyingKey}
              autofocus
            />
            {#if adminError}<div class="ap-err">{adminError}</div>{/if}
            <div class="ap-btns">
              {#if appState.adminVerified}
                <button type="button" class="btn btn-sm btn-ghost" onclick={lockAdminKey}>Lock session</button>
              {/if}
              <button type="submit" class="btn btn-sm btn-white" disabled={verifyingKey}>
                {#if verifyingKey}<span class="spinner"></span>{/if}
                {verifyingKey ? 'Verifying' : 'Verify key'}
              </button>
            </div>
          </form>
        {/if}
      </div>

      <button
        type="button"
        class="shield"
        class:armed
        role="switch"
        aria-checked={armed}
        aria-label="Security shield"
        onclick={handleShieldClick}
      >
        <span class="sh-track"><span class="sh-knob">{#if armed}<Shield size={10} strokeWidth={2.4} />{:else}<ShieldOff size={10} strokeWidth={2.4} />{/if}</span></span>
        <span class="sh-label mono">{armed ? 'ARMED' : 'BYPASSED'}</span>
      </button>
    </div>
  </header>

  {#if !armed}
    <div class="breach-bar" transition:fly={{ y: -8, duration: 260 }}>
      <span class="dot live" style="color: var(--danger)"></span>
      <span class="mono bb-k">PERIMETER BYPASSED</span>
      <span class="bb-t">Inline layers L1–L5 disabled for comparison. L6 Sentinel is still observing asynchronously.</span>
      <button type="button" class="btn btn-xs btn-danger" onclick={() => executeShieldToggle(true)}>Re-arm shield</button>
    </div>
  {/if}

  <main class="stage">
    {#key appState.activeTab}
      <div class="view" in:fly={{ y: 10, duration: 420, opacity: 0 }}>
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
      </div>
    {/key}
  </main>
</div>

<!-- Toasts -->
<div class="toasts" aria-live="polite">
  {#each appState.toasts as t (t.id)}
    <div class="toast glass {t.kind}" in:fly={{ y: 16, duration: 280 }} out:fade={{ duration: 160 }}>
      <span class="t-ico">
        {#if t.kind === 'success'}<Check size={13} strokeWidth={2.4} />
        {:else if t.kind === 'warn'}<TriangleAlert size={13} strokeWidth={2} />
        {:else if t.kind === 'error'}<ShieldX size={13} strokeWidth={2} />
        {:else}<Info size={13} strokeWidth={2} />{/if}
      </span>
      <div class="t-txt">
        <div class="t-title">{t.title}</div>
        {#if t.detail}<div class="t-det">{t.detail}</div>{/if}
      </div>
    </div>
  {/each}
</div>

<ConfirmModal
  open={showBypassConfirm}
  title="Bypass the security shield?"
  message="This disables inline prompt-injection filtering, DPDP PII masking, and RBI circuit-breaker checks so you can compare unprotected agent behaviour. Layer 6 Sentinel keeps watching asynchronously."
  confirmLabel="Bypass shield"
  tone="crimson"
  onCancel={() => (showBypassConfirm = false)}
  onConfirm={() => {
    showBypassConfirm = false;
    executeShieldToggle(false);
  }}
/>

<CommandPalette open={paletteOpen} {commands} onClose={() => (paletteOpen = false)} />

{#if showHelp}
  <div class="help-back" transition:fade={{ duration: 160 }}>
    <button type="button" class="help-dismiss" aria-label="Close help" onclick={() => (showHelp = false)}></button>
    <div class="help glass" role="dialog" aria-modal="true" aria-label="Quick guide" transition:scale={{ start: 0.97, duration: 220 }}>
      <div class="h-head">
        <div>
          <div class="eyebrow">Field guide</div>
          <h3 class="serif">Operating the perimeter</h3>
        </div>
        <button type="button" class="btn btn-icon btn-sm btn-ghost" onclick={() => (showHelp = false)} aria-label="Close">
          <X size={15} />
        </button>
      </div>
      <div class="h-list">
        {#each [
          { k: '1', t: 'Overview', d: 'Latency percentiles, the six-layer mesh, and a live forensic event stream.' },
          { k: '2', t: 'Chats', d: 'Watch customer sessions, inspect tool calls, take over, approve held ₹5,000+ refunds.' },
          { k: '3', t: 'Sandbox', d: 'One-click adversarial scenarios — raw input vs. what the model actually receives.' },
          { k: '4', t: 'Sentinel', d: 'Async watcher catching multi-turn drift and reverting rogue payouts. Exports.' },
          { k: '5', t: 'Settings', d: 'RBI limits, DPDP masking rules, Gemini key and admin lock — applied live.' },
        ] as row}
          <div class="h-row">
            <span class="kbd">{row.k}</span>
            <span class="h-t">{row.t}</span>
            <span class="h-d">{row.d}</span>
          </div>
        {/each}
      </div>
      <div class="h-foot">
        <span><span class="kbd">Ctrl</span><span class="kbd">K</span> command palette</span>
        <span><span class="kbd">T</span> theme</span>
        <span><span class="kbd">D</span> demo mode</span>
        <span><span class="kbd">?</span> this guide</span>
        <span><span class="kbd">Esc</span> close</span>
      </div>
    </div>
  </div>
{/if}

<style>
  /* ---------- Ambient canvas ---------- */
  .ambient {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    overflow: hidden;
  }
  .aurora {
    position: absolute;
    top: -42vh;
    left: 50%;
    width: 140vw;
    height: 80vh;
    transform: translateX(-50%);
    background:
      radial-gradient(40% 50% at 50% 60%, var(--aurora-1), transparent 70%),
      radial-gradient(30% 40% at 30% 70%, var(--aurora-2), transparent 70%);
    transition: opacity 900ms var(--ease);
  }
  .ambient::after {
    content: '';
    position: absolute;
    inset: 0;
    opacity: 0;
    background:
      radial-gradient(60% 45% at 50% -5%, rgba(var(--danger-rgb), 0.16), transparent 70%),
      radial-gradient(120% 120% at 50% 50%, transparent 60%, rgba(var(--danger-rgb), 0.08));
    transition: opacity 900ms var(--ease);
  }
  .ambient.breach .aurora {
    opacity: 0;
  }
  .ambient.breach::after {
    opacity: 1;
  }
  .dots {
    position: absolute;
    inset: 0;
    background-image: radial-gradient(var(--dot-grid) 1px, transparent 1.2px);
    background-size: 22px 22px;
    mask-image: radial-gradient(ellipse 70% 55% at 50% 0%, #000 10%, transparent 75%);
    -webkit-mask-image: radial-gradient(ellipse 70% 55% at 50% 0%, #000 10%, transparent 75%);
  }
  .grain {
    position: absolute;
    inset: 0;
    opacity: 0.045;
    mix-blend-mode: overlay;
    background-image: url("data:image/svg+xml;utf8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='180' height='180'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
  }

  /* ---------- Shell ---------- */
  .shell {
    position: relative;
    z-index: 1;
    height: 100vh;
    display: flex;
    flex-direction: column;
  }

  .topbar {
    position: relative;
    z-index: 30;
    flex: none;
    height: 60px;
    padding: 0 20px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(max-content, 1fr);
    align-items: center;
    gap: 20px;
    border-bottom: 1px solid var(--line);
    background: var(--topbar-bg);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
  }
  .tb-left {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }
  .brand {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    background: none;
    border: none;
    cursor: pointer;
    padding: 4px 2px;
  }
  .wordmark {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.03em;
  }
  .sep {
    width: 1px;
    height: 18px;
    background: var(--line-2);
    transform: rotate(18deg);
  }
  .policy {
    font-size: 10.5px;
    color: var(--fg-3);
    letter-spacing: 0.04em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Segmented nav */
  .nav {
    position: relative;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border-radius: 12px;
    background: var(--well);
    border: 1px solid var(--line);
  }
  .indicator {
    position: absolute;
    top: 3px;
    bottom: 3px;
    left: 0;
    border-radius: 9px;
    background: var(--nav-ind-bg);
    box-shadow: var(--nav-ind-shadow);
    opacity: 0;
  }
  .indicator.ready {
    opacity: 1;
    transition: transform 420ms var(--ease-spring), width 420ms var(--ease-spring), opacity 200ms;
  }
  .nav-item {
    position: relative;
    z-index: 1;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 11px;
    border: none;
    border-radius: 9px;
    background: transparent;
    color: var(--fg-3);
    font-size: 12.5px;
    font-weight: 500;
    white-space: nowrap;
    cursor: pointer;
    transition: color 200ms var(--ease);
  }
  .nav-item:hover {
    color: var(--fg-2);
  }
  .nav-item.active {
    color: var(--fg);
  }
  .badge {
    display: grid;
    place-items: center;
    min-width: 17px;
    height: 17px;
    padding: 0 5px;
    border-radius: 99px;
    font-size: 10px;
    font-weight: 600;
    background: var(--warn);
    color: #140c00;
    box-shadow: 0 0 12px -2px rgba(var(--warn-rgb), 0.7);
  }

  .tb-right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    flex-shrink: 0;
  }
  .cmdk {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 7px 0 9px;
    border-radius: 9px;
    border: 1px solid var(--line);
    background: var(--s-1);
    color: var(--fg-3);
    font-size: 12px;
    white-space: nowrap;
    cursor: pointer;
    transition: border-color 160ms, color 160ms, background 160ms;
  }
  .cmdk:hover {
    border-color: var(--line-2);
    color: var(--fg-2);
    background: var(--s-2);
  }
  .ck-l {
    margin-right: 2px;
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 10.5px;
    letter-spacing: 0.08em;
    color: var(--fg-2);
    padding: 0 4px;
  }
  .engine {
    color: var(--fg-4);
  }
  .officer {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 8px 0 4px;
    border-radius: 99px;
    border: 1px solid var(--line);
    background: var(--s-1);
    cursor: pointer;
    transition: border-color 160ms;
  }
  .officer:hover {
    border-color: var(--line-2);
  }
  .av {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 9.5px;
    font-weight: 600;
    color: var(--solid-fg);
    background: linear-gradient(135deg, var(--accent), var(--av-grad-end));
  }
  .officer select {
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    border: none;
    outline: none;
    color: var(--fg-2);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    padding-right: 2px;
  }
  .officer select option {
    background: var(--bg-2);
    color: var(--fg);
  }

  .admin-wrap {
    position: relative;
  }
  .theme-btn,
  .key-btn {
    position: relative;
    height: 30px;
    width: 30px;
  }
  .demo-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 30px;
    padding: 0 10px;
    border-radius: 9px;
    border: 1px solid var(--line);
    background: var(--s-1);
    color: var(--fg-3);
    font-size: 11.5px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: border-color 160ms var(--ease), color 160ms var(--ease), background 160ms var(--ease);
  }
  .demo-btn:hover {
    border-color: var(--line-2);
    color: var(--fg-2);
    background: var(--s-2);
  }
  .demo-btn.active {
    border-color: rgba(var(--accent-rgb), 0.45);
    background: rgba(var(--accent-rgb), 0.12);
    color: var(--accent);
    font-weight: 600;
  }
  :global(:root[data-demo='true']) .policy,
  :global(:root[data-demo='true']) .sep,
  :global(:root[data-demo='true']) .cmdk,
  :global(:root[data-demo='true']) .officer,
  :global(:root[data-demo='true']) .admin-wrap,
  :global(:root[data-demo='true']) .toasts {
    display: none;
  }
  :global(:root[data-demo='true']) .stage {
    padding: 26px 34px 28px;
  }
  :global(:root[data-demo='true']) .wordmark {
    font-size: 16.5px;
  }
  :global(:root[data-demo='true']) .nav-item {
    font-size: 13.5px;
    height: 34px;
    padding: 0 14px;
  }
  .key-btn.verified {
    color: var(--accent);
  }
  .kdot {
    position: absolute;
    top: 5px;
    right: 5px;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent);
  }
  .admin-pop {
    position: absolute;
    right: 0;
    top: calc(100% + 10px);
    width: 300px;
    padding: 16px;
    border-radius: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    box-shadow: var(--shadow-pop);
  }
  .ap-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .ap-title {
    font-size: 21px;
  }
  .ap-sub {
    font-size: 12px;
    color: var(--fg-3);
    line-height: 1.5;
  }
  .ap-err {
    font-size: 11.5px;
    color: var(--danger);
  }
  .ap-btns {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-top: 2px;
  }

  /* Shield switch */
  .shield {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    height: 32px;
    padding: 0 12px 0 4px;
    border-radius: 99px;
    cursor: pointer;
    border: 1px solid rgba(var(--danger-rgb), 0.4);
    background: rgba(var(--danger-rgb), 0.08);
    color: var(--danger);
    transition: border-color 300ms var(--ease), background 300ms var(--ease), color 300ms var(--ease),
      box-shadow 300ms var(--ease);
  }
  .shield.armed {
    border-color: rgba(var(--accent-rgb), 0.3);
    background: rgba(var(--accent-rgb), 0.06);
    color: var(--accent);
    box-shadow: 0 0 24px -8px rgba(var(--accent-rgb), 0.6);
  }
  .shield:hover {
    box-shadow: 0 0 28px -6px currentColor;
  }
  .sh-track {
    position: relative;
    width: 40px;
    height: 24px;
    border-radius: 99px;
    background: var(--well);
    border: 1px solid var(--line);
  }
  .sh-knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--danger);
    color: #ffffff;
    transition: transform 420ms var(--ease-spring), background 300ms;
  }
  .armed .sh-knob {
    transform: translateX(16px);
    background: var(--accent);
    color: var(--solid-fg);
    box-shadow: 0 0 10px rgba(var(--accent-rgb), 0.7);
  }
  .sh-label {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.12em;
  }

  /* Breach banner */
  .breach-bar {
    position: relative;
    z-index: 20;
    flex: none;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 20px;
    font-size: 12px;
    color: var(--fg-2);
    background: linear-gradient(90deg, rgba(var(--danger-rgb), 0.14), rgba(var(--danger-rgb), 0.03) 60%, transparent);
    border-bottom: 1px solid rgba(var(--danger-rgb), 0.25);
  }
  .bb-k {
    color: var(--danger);
    font-size: 10.5px;
    letter-spacing: 0.14em;
    font-weight: 600;
  }
  .bb-t {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .stage {
    flex: 1;
    min-height: 0;
    padding: 20px 24px 22px;
    overflow: hidden;
  }
  .view {
    height: 100%;
    min-height: 0;
  }

  /* ---------- Toasts ---------- */
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 99;
    width: 340px;
    display: flex;
    flex-direction: column-reverse;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    --c: var(--info);
    --c-rgb: var(--info-rgb);
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    gap: 11px;
    padding: 12px 14px;
    border-radius: 14px;
    box-shadow: var(--shadow-pop);
  }
  .toast.success { --c: var(--accent); --c-rgb: var(--accent-rgb); }
  .toast.warn { --c: var(--warn); --c-rgb: var(--warn-rgb); }
  .toast.error { --c: var(--danger); --c-rgb: var(--danger-rgb); }
  .t-ico {
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: rgba(var(--c-rgb), 0.12);
    border: 1px solid rgba(var(--c-rgb), 0.25);
  }
  .t-txt {
    min-width: 0;
    padding-top: 1px;
  }
  .t-title {
    font-size: 12.5px;
    font-weight: 500;
  }
  .t-det {
    margin-top: 2px;
    font-size: 11.5px;
    color: var(--fg-3);
    word-break: break-word;
  }

  /* ---------- Help ---------- */
  .help-back {
    position: fixed;
    inset: 0;
    z-index: 92;
    display: grid;
    place-items: center;
    background: var(--backdrop-bg);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
  }
  .help-dismiss {
    position: fixed;
    inset: 0;
    border: none;
    background: transparent;
  }
  .help {
    position: relative;
    z-index: 1;
    width: min(560px, 92vw);
    border-radius: 22px;
    padding: 24px;
    box-shadow: var(--shadow-pop);
  }
  .h-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 18px;
  }
  .h-head h3 {
    font-size: 28px;
    line-height: 1.15;
    margin-top: 6px;
  }
  .h-list {
    display: flex;
    flex-direction: column;
  }
  .h-row {
    display: grid;
    grid-template-columns: 24px 80px 1fr;
    align-items: baseline;
    gap: 12px;
    padding: 11px 0;
    border-top: 1px solid var(--line);
  }
  .h-t {
    font-weight: 500;
  }
  .h-d {
    font-size: 12.5px;
    color: var(--fg-3);
  }
  .h-foot {
    display: flex;
    gap: 18px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
    font-size: 11.5px;
    color: var(--fg-3);
  }
  .h-foot > span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  /* ---------- Responsive ---------- */
  @media (max-width: 1480px) {
    .policy,
    .sep,
    .engine {
      display: none;
    }
  }
  @media (max-width: 1320px) {
    .ck-l {
      display: none;
    }
  }
  @media (max-width: 1120px) {
    .nl {
      display: none;
    }
    .live span:not(.dot) {
      display: none;
    }
    .officer select {
      width: 0;
      padding: 0;
      opacity: 0;
      position: absolute;
      inset: 0;
      width: 100%;
    }
    .officer {
      padding: 0 4px;
    }
  }
  @media (max-width: 760px) {
    .wordmark,
    .cmdk,
    .sh-label {
      display: none;
    }
    .stage {
      padding: 14px;
    }
  }
</style>
