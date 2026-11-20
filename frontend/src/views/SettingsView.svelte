<script lang="ts">
  import { onMount } from 'svelte';
  import { api, setAdminKey, type PublicSettingsView } from '../lib/api';
  import { appState } from '../lib/stores/appState.svelte';
  import {
    Save,
    Scale,
    KeyRound,
    Fingerprint,
    Sparkles,
    Check,
  } from 'lucide-svelte';

  let settings = $state<PublicSettingsView | null>(null);
  let geminiKeyInput = $state('');
  let adminKeyInput = $state('');
  let saving = $state(false);

  async function load() {
    try {
      settings = await api.getSettings();
    } catch (e: unknown) {
      appState.pushToast('error', 'Failed to load settings', (e as Error).message);
    }
  }

  function isAuthErr(msg: string): boolean {
    return msg.includes('Unauthorized') || msg.includes('Admin Key') || msg.includes('401');
  }

  async function saveAll() {
    if (!settings) return;
    if (
      !Number.isFinite(settings.statutory_limits.max_auto_refund_amount) ||
      settings.statutory_limits.max_auto_refund_amount < 1 ||
      !Number.isFinite(settings.statutory_limits.max_24h_refund_total) ||
      settings.statutory_limits.max_24h_refund_total < 1 ||
      !Number.isFinite(settings.statutory_limits.max_credit_limit_multiplier) ||
      settings.statutory_limits.max_credit_limit_multiplier < 1 ||
      !Number.isFinite(settings.rate_limit_rpm) ||
      settings.rate_limit_rpm < 1
    ) {
      appState.pushToast(
        'error',
        'Invalid policy limits',
        'Refund caps, credit multiplier, and rate limit must all be >= 1.',
      );
      return;
    }

    saving = true;
    try {
      const newAdmin = adminKeyInput.trim();
      const patch: Record<string, unknown> = {
        policy_preset: settings.policy_preset,
        rate_limit_rpm: settings.rate_limit_rpm,
        agent_model: settings.agent_model,
        decision_model: settings.decision_model,
        decisions_mode: settings.decisions_mode,
        sentinel_enabled: settings.sentinel_enabled,
        sentinel_model: settings.sentinel_model,
        auto_quarantine_threshold: settings.auto_quarantine_threshold,
        auto_revert_rogue_payouts: settings.auto_revert_rogue_payouts,
        statutory_limits: settings.statutory_limits,
        dpdp_act: settings.dpdp_act,
      };
      if (geminiKeyInput.trim()) patch.gemini_api_key = geminiKeyInput.trim();
      if (newAdmin) patch.admin_key = newAdmin;

      const res = await api.updateSettings(patch);
      settings = res.settings;
      if (newAdmin) {
        setAdminKey(newAdmin);
        appState.adminVerified = true;
      }
      geminiKeyInput = '';
      adminKeyInput = '';
      appState.pushToast('success', 'Configuration applied live');
      await appState.refreshAll();
    } catch (e: unknown) {
      const msg = (e as Error).message;
      if (isAuthErr(msg)) {
        appState.requestAdminUnlock(msg);
      } else {
        appState.pushToast('error', 'Save failed', msg);
      }
    } finally {
      saving = false;
    }
  }

  async function applyPreset(preset: 'RBI_DPDP_FINTECH_V1' | 'STRICT_BANKING') {
    try {
      await api.applyPolicyPreset(preset);
      await load();
      await appState.refreshAll();
      appState.pushToast('success', `Applied ${preset}`);
    } catch (e: unknown) {
      const msg = (e as Error).message;
      if (isAuthErr(msg)) {
        appState.requestAdminUnlock(msg);
      } else {
        appState.pushToast('error', 'Preset error', msg);
      }
    }
  }

  onMount(load);
</script>

<div class="settings-page">
  {#if settings}
    <header class="card head-bar">
      <div>
        <div class="eyebrow">Control Plane · Hot-Reload Configuration</div>
        <h2>Statutory Policy & Runtime Parameters</h2>
        <p class="muted">
          Changes propagate immediately across all six Rust inspection layers and persist locally.
        </p>
      </div>

      <div class="actions">
        <div class="preset-group">
          <button
            type="button"
            class="preset-pill"
            class:active={settings.policy_preset === 'RBI_DPDP_FINTECH_V1'}
            onclick={() => applyPreset('RBI_DPDP_FINTECH_V1')}
          >
            {#if settings.policy_preset === 'RBI_DPDP_FINTECH_V1'}<Check size={12} />{/if}
            <span>RBI Standard</span>
          </button>
          <button
            type="button"
            class="preset-pill"
            class:active={settings.policy_preset === 'STRICT_BANKING'}
            onclick={() => applyPreset('STRICT_BANKING')}
          >
            {#if settings.policy_preset === 'STRICT_BANKING'}<Check size={12} />{/if}
            <span>Strict Banking</span>
          </button>
        </div>

        <button type="button" class="btn btn-solid" onclick={saveAll} disabled={saving}>
          {#if saving}<span class="spinner"></span>{:else}<Save size={14} />{/if}
          <span>{saving ? 'Applying…' : 'Save Changes'}</span>
        </button>
      </div>
    </header>

    <div class="cfg-grid">
      <!-- 01: Statutory RBI Circuit Breaker -->
      <section class="card cfg-card">
        <header class="cc-head">
          <span class="cc-ico"><Scale size={15} /></span>
          <div>
            <div class="eyebrow">Layer 4 · Deterministic Policy</div>
            <h3>RBI Statutory Circuit Breaker</h3>
          </div>
        </header>

        <div class="field-grid">
          <label class="field">
            <span class="f-lbl">Single Auto-Refund Cap (₹)</span>
            <input
              type="number"
              class="input mono"
              bind:value={settings.statutory_limits.max_auto_refund_amount}
            />
          </label>
          <label class="field">
            <span class="f-lbl">24h Rolling Smurfing Cap (₹)</span>
            <input
              type="number"
              class="input mono"
              bind:value={settings.statutory_limits.max_24h_refund_total}
            />
          </label>
          <label class="field">
            <span class="f-lbl">Max Credit Multiplier (×)</span>
            <input
              type="number"
              step="0.05"
              class="input mono"
              bind:value={settings.statutory_limits.max_credit_limit_multiplier}
            />
          </label>
          <label class="field">
            <span class="f-lbl">Gateway Rate Limit (RPM)</span>
            <input type="number" class="input mono" bind:value={settings.rate_limit_rpm} />
          </label>
        </div>

        <div class="switch-list">
          <label class="sw-row">
            <div>
              <div class="sw-title">Block autonomous tools on frozen accounts</div>
              <div class="sw-sub">Holds any balance or profile mutation when account is frozen</div>
            </div>
            <input
              type="checkbox"
              class="switch"
              bind:checked={settings.statutory_limits.prohibit_frozen_accounts}
            />
          </label>

          <label class="sw-row">
            <div>
              <div class="sw-title">Enforce KYC state-machine lockdown</div>
              <div class="sw-sub">Prevents AI from jumping FLAGGED or REJECTED profiles to VERIFIED</div>
            </div>
            <input
              type="checkbox"
              class="switch"
              bind:checked={settings.statutory_limits.enforce_kyc_transition_rules}
            />
          </label>
        </div>
      </section>

      <!-- 02: Google Gemini & Decisions API -->
      <section class="card cfg-card">
        <header class="cc-head">
          <span class="cc-ico"><Sparkles size={15} /></span>
          <div>
            <div class="eyebrow">Layer 3 & Layer 6 · Intelligence</div>
            <h3>Google Gemini & Decisions Engine</h3>
          </div>
          <span class="tag {settings.gemini_api_key_configured ? 'ok' : 'warn'} dotted" style="margin-left: auto;">
            {settings.gemini_api_key_configured ? 'Gemini Live' : 'Simulator'}
          </span>
        </header>

        <div class="field-grid">
          <label class="field">
            <span class="f-lbl">Decisions Guardrail Mode</span>
            <select class="select" bind:value={settings.decisions_mode}>
              <option value="Smart">Smart (Fast-Path + Decisions)</option>
              <option value="Strict">Strict (Gate Every Turn)</option>
              <option value="Off">Off (Regex Only)</option>
            </select>
          </label>
          <label class="field">
            <span class="f-lbl">Fintech Agent Model</span>
            <input
              type="text"
              list="agent-model-presets"
              class="input mono"
              bind:value={settings.agent_model}
            />
          </label>
          <label class="field">
            <span class="f-lbl">Inline Decision Model (L3)</span>
            <input
              type="text"
              list="decision-model-presets"
              class="input mono"
              bind:value={settings.decision_model}
            />
          </label>
          <label class="field">
            <span class="f-lbl">Async Sentinel Model (L6)</span>
            <input
              type="text"
              list="sentinel-model-presets"
              class="input mono"
              bind:value={settings.sentinel_model}
            />
          </label>
        </div>

        <datalist id="agent-model-presets">
          <option value="gemini-3.5-flash-lite"></option>
          <option value="gemini-3-flash-preview"></option>
          <option value="gemini-3.1-flash-lite"></option>
          <option value="gemini-4-hydrogen"></option>
        </datalist>
        <datalist id="decision-model-presets">
          <option value="gemini-decision-1"></option>
        </datalist>
        <datalist id="sentinel-model-presets">
          <option value="gemini-3-flash-preview"></option>
          <option value="gemini-3.5-flash-lite"></option>
          <option value="gemini-3.1-flash-lite"></option>
          <option value="gemini-4-carbon"></option>
          <option value="gemini-4-argon"></option>
        </datalist>

        <label class="field">
          <div class="f-row">
            <span class="f-lbl">Google Gemini API Key</span>
            <span class="mono muted" style="font-size: 11px;">
              {settings.gemini_api_key_configured
                ? `Active: ${settings.gemini_api_key_masked}`
                : 'Unset — deterministic simulator active'}
            </span>
          </div>
          <input
            type="password"
            class="input mono"
            placeholder="Paste AIza… key to enable live Gemini inference"
            bind:value={geminiKeyInput}
          />
        </label>

        <div class="switch-list">
          <label class="sw-row">
            <div>
              <div class="sw-title">Enable Layer 6 Async AI Sentinel</div>
              <div class="sw-sub">Out-of-band multi-turn trajectory auditing over Tokio channel</div>
            </div>
            <input type="checkbox" class="switch" bind:checked={settings.sentinel_enabled} />
          </label>

          <label class="sw-row">
            <div>
              <div class="sw-title">Auto-revert unauthorized unprotected payouts</div>
              <div class="sw-sub">Automatically claws back rogue payouts detected when shield is bypassed</div>
            </div>
            <input type="checkbox" class="switch" bind:checked={settings.auto_revert_rogue_payouts} />
          </label>
        </div>
      </section>

      <!-- 03: DPDP PII Masking -->
      <section class="card cfg-card">
        <header class="cc-head">
          <span class="cc-ico warn"><Fingerprint size={15} /></span>
          <div>
            <div class="eyebrow">Layer 2 & Layer 5 · Data Minimization</div>
            <h3>DPDP Act 2023 Ephemeral PII Vault</h3>
          </div>
        </header>

        <div class="field-grid">
          <label class="field">
            <span class="f-lbl">Inbound Token Format</span>
            <select class="select mono" bind:value={settings.dpdp_act.default_token_format}>
              <option value="AngleUuid">&lt;PII:TYPE:128-bit UUID&gt;</option>
              <option value="BracketIndexed">[TYPE_TOKEN_01]</option>
            </select>
          </label>
          <label class="field">
            <span class="f-lbl">Outbound Egress Mode</span>
            <select class="select" bind:value={settings.dpdp_act.outbound_pii_mode}>
              <option value="MaskFull">Mask Full ([REDACTED:TYPE])</option>
              <option value="Last4">Last 4 Visible (XXXX-1234)</option>
              <option value="RehydrateAuthorized">Authorized Session Rehydrate</option>
            </select>
          </label>
        </div>

        <div class="pii-toggles">
          <label class="pii-tog">
            <span>Aadhaar (12-digit)</span>
            <input type="checkbox" class="switch" bind:checked={settings.dpdp_act.mask_aadhaar} />
          </label>
          <label class="pii-tog">
            <span>PAN Card</span>
            <input type="checkbox" class="switch" bind:checked={settings.dpdp_act.mask_pan} />
          </label>
          <label class="pii-tog">
            <span>Phone (+91)</span>
            <input type="checkbox" class="switch" bind:checked={settings.dpdp_act.mask_phone} />
          </label>
          <label class="pii-tog">
            <span>Card (Luhn)</span>
            <input type="checkbox" class="switch" bind:checked={settings.dpdp_act.mask_card} />
          </label>
          <label class="pii-tog">
            <span>IFSC Code</span>
            <input type="checkbox" class="switch" bind:checked={settings.dpdp_act.mask_ifsc} />
          </label>
          <label class="pii-tog">
            <span>UPI VPA</span>
            <input type="checkbox" class="switch" bind:checked={settings.dpdp_act.mask_upi} />
          </label>
          <label class="pii-tog">
            <span>OTP / CVV / A/C</span>
            <input type="checkbox" class="switch" bind:checked={settings.dpdp_act.mask_secrets} />
          </label>
          <label class="pii-tog">
            <span>Strict Verhoeff</span>
            <input type="checkbox" class="switch" bind:checked={settings.dpdp_act.strict_aadhaar_verhoeff} />
          </label>
        </div>
      </section>

      <!-- 04: Staff Perimeter Admin Lock -->
      <section class="card cfg-card">
        <header class="cc-head">
          <span class="cc-ico staff"><KeyRound size={15} /></span>
          <div>
            <div class="eyebrow">Access Control · Zero-Trust Staff Gate</div>
            <h3>Staff Perimeter Admin Lock</h3>
          </div>
          <span class="tag {settings.admin_key_configured ? 'ok' : ''} dotted" style="margin-left: auto;">
            {settings.admin_key_configured ? 'Enforced' : 'Open Dev Mode'}
          </span>
        </header>

        <p class="muted" style="font-size: 12.5px; line-height: 1.55;">
          When configured, an Admin Key is required to disable the 6-layer shield, modify statutory policies, export compliance reports, or unmask customer PII in Staff Chats.
        </p>

        <label class="field">
          <div class="f-row">
            <span class="f-lbl">Set or Rotate Admin Key</span>
            <span class="mono muted" style="font-size: 11px;">
              {settings.admin_key_configured ? 'LOCKED & ENFORCED' : 'No key set'}
            </span>
          </div>
          <input
            type="password"
            class="input mono"
            placeholder="Enter new AEGIS_ADMIN_KEY…"
            bind:value={adminKeyInput}
          />
        </label>

        <div class="meta-box">
          <div class="eyebrow">Whitelisted Agent Tools ({settings.statutory_limits.allowed_tools.length})</div>
          <div class="tool-chips">
            {#each settings.statutory_limits.allowed_tools as t}
              <span class="tag plain mono">{t}()</span>
            {/each}
          </div>
        </div>
      </section>
    </div>
  {:else}
    <div class="card" style="padding: 48px; text-align: center;">
      <span class="muted">Loading runtime configuration…</span>
    </div>
  {/if}
</div>

<style>
  .settings-page {
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: 100%;
    overflow-y: auto;
    padding-right: 4px;
  }

  .head-bar {
    padding: 18px 22px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
    background:
      radial-gradient(60% 140% at 0% 0%, rgba(var(--accent-rgb), 0.07), transparent 70%),
      var(--card-bg);
  }
  .head-bar h2 {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.015em;
    margin-top: 2px;
  }
  .head-bar p {
    font-size: 12.5px;
    margin-top: 2px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .preset-group {
    display: inline-flex;
    padding: 3px;
    border-radius: 10px;
    background: var(--well);
    border: 1px solid var(--line);
  }
  .preset-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border-radius: 7px;
    border: none;
    background: transparent;
    color: var(--fg-3);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: color 140ms, background 140ms;
  }
  .preset-pill:hover {
    color: var(--fg-2);
  }
  .preset-pill.active {
    background: var(--pill-active-bg);
    color: var(--fg);
    box-shadow: var(--pill-active-shadow);
  }

  /* Grid */
  .cfg-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
  }
  .cfg-card {
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .cc-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--line);
  }
  .cc-ico {
    width: 34px;
    height: 34px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    color: var(--accent);
    background: rgba(var(--accent-rgb), 0.1);
    border: 1px solid rgba(var(--accent-rgb), 0.28);
  }
  .cc-ico.warn {
    color: var(--warn);
    background: rgba(var(--warn-rgb), 0.1);
    border-color: rgba(var(--warn-rgb), 0.28);
  }
  .cc-ico.staff {
    color: var(--staff);
    background: rgba(var(--staff-rgb), 0.1);
    border-color: rgba(var(--staff-rgb), 0.28);
  }
  .cc-head h3 {
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.01em;
    margin-top: 1px;
  }

  .field-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .f-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .f-lbl {
    font-size: 11.5px;
    font-weight: 500;
    color: var(--fg-2);
  }

  .switch-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: auto;
  }
  .sw-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--s-1);
    border: 1px solid var(--line);
    cursor: pointer;
    transition: border-color 140ms;
  }
  .sw-row:hover {
    border-color: var(--line-2);
  }
  .sw-title {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--fg);
  }
  .sw-sub {
    font-size: 11px;
    color: var(--fg-3);
    margin-top: 1px;
  }

  .pii-toggles {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 7px;
  }
  .pii-tog {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 11px;
    border-radius: 9px;
    background: var(--s-1);
    border: 1px solid var(--line);
    font-size: 12px;
    color: var(--fg);
    cursor: pointer;
    transition: border-color 140ms;
  }
  .pii-tog:hover {
    border-color: var(--line-2);
  }

  .meta-box {
    margin-top: auto;
    padding: 12px;
    border-radius: 11px;
    background: var(--s-1);
    border: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .tool-chips {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  @media (max-width: 980px) {
    .cfg-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
