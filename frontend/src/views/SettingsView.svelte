<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type PublicSettingsView } from '../lib/api';
  import { appState } from '../lib/stores/appState.svelte';
  import { fade } from 'svelte/transition';
  import { Save, Shield, KeyRound, EyeOff, Brain } from 'lucide-svelte';

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

  async function saveAll() {
    if (!settings) return;
    saving = true;
    try {
      const patch: Record<string, unknown> = {
        policy_preset: settings.policy_preset,
        rate_limit_rpm: settings.rate_limit_rpm,
        agent_model: settings.agent_model,
        decisions_mode: settings.decisions_mode,
        sentinel_enabled: settings.sentinel_enabled,
        sentinel_model: settings.sentinel_model,
        auto_quarantine_threshold: settings.auto_quarantine_threshold,
        auto_revert_rogue_payouts: settings.auto_revert_rogue_payouts,
        statutory_limits: settings.statutory_limits,
        dpdp_act: settings.dpdp_act,
      };
      if (geminiKeyInput.trim()) patch.gemini_api_key = geminiKeyInput.trim();
      if (adminKeyInput.trim()) patch.admin_key = adminKeyInput.trim();

      const res = await api.updateSettings(patch);
      settings = res.settings;
      geminiKeyInput = '';
      adminKeyInput = '';
      appState.pushToast('success', 'Settings Saved & Applied Live');
      await appState.refreshAll();
    } catch (e: unknown) {
      appState.pushToast('error', 'Save Failed', (e as Error).message);
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
      appState.pushToast('error', 'Preset Error', (e as Error).message);
    }
  }

  onMount(load);
</script>

<div class="settings-page" in:fade={{ duration: 160 }}>
  {#if settings}
    <div class="panel head-bar">
      <div>
        <h2>Policy & Runtime Configuration</h2>
        <p class="muted">Changes apply immediately across all 6 security layers and persist locally.</p>
      </div>
      <div class="actions">
        <button type="button" class="btn" onclick={() => applyPreset('RBI_DPDP_FINTECH_V1')}>
          RBI Standard Preset
        </button>
        <button type="button" class="btn" onclick={() => applyPreset('STRICT_BANKING')}>
          Strict Banking Preset
        </button>
        <button type="button" class="btn btn-primary" onclick={saveAll} disabled={saving}>
          <Save size={14} /> {saving ? 'Saving...' : 'Save Changes'}
        </button>
      </div>
    </div>

    <div class="cfg-grid">
      <!-- Statutory RBI Limits -->
      <section class="panel cfg-card">
        <h3><Shield size={15} color="#22d3ee" /> RBI Statutory Circuit Breaker</h3>
        <div class="field-grid">
          <label>
            <span>Single Auto-Refund Cap (₹)</span>
            <input type="number" class="input mono" bind:value={settings.statutory_limits.max_auto_refund_amount} />
          </label>
          <label>
            <span>24h Rolling Smurfing Cap (₹)</span>
            <input type="number" class="input mono" bind:value={settings.statutory_limits.max_24h_refund_total} />
          </label>
          <label>
            <span>Max Credit Multiplier (×)</span>
            <input type="number" step="0.05" class="input mono" bind:value={settings.statutory_limits.max_credit_limit_multiplier} />
          </label>
          <label>
            <span>Rate Limit (Req / Min)</span>
            <input type="number" class="input mono" bind:value={settings.rate_limit_rpm} />
          </label>
        </div>
        <div class="checks">
          <label class="chk">
            <input type="checkbox" bind:checked={settings.statutory_limits.prohibit_frozen_accounts} />
            <span>Block autonomous tools on frozen accounts</span>
          </label>
          <label class="chk">
            <input type="checkbox" bind:checked={settings.statutory_limits.enforce_kyc_transition_rules} />
            <span>Enforce KYC state-machine lockdown</span>
          </label>
        </div>
      </section>

      <!-- Google Gemini & Decisions API -->
      <section class="panel cfg-card">
        <h3><Brain size={15} color="#10b981" /> Google Gemini & Decisions API</h3>
        <div class="field-grid">
          <label>
            <span>Gemini Decisions API Mode</span>
            <select class="select" bind:value={settings.decisions_mode}>
              <option value="Smart">Smart (Fast-Path + Decisions Gating)</option>
              <option value="Strict">Strict (Gate Every Prompt)</option>
              <option value="Off">Off</option>
            </select>
          </label>
          <label>
            <span>Agent & Sentinel Model</span>
            <input type="text" class="input mono" bind:value={settings.agent_model} />
          </label>
        </div>

        <label>
          <span>
            Gemini API Key
            <b class="mono" style="color: {settings.gemini_api_key_configured ? '#10b981' : '#f59e0b'};">
              ({settings.gemini_api_key_configured ? settings.gemini_api_key_masked : 'Not Configured — Simulator Mode'})
            </b>
          </span>
          <input
            type="password"
            class="input mono"
            placeholder="Paste new AIza... key to enable live Gemini"
            bind:value={geminiKeyInput}
          />
        </label>

        <div class="checks">
          <label class="chk">
            <input type="checkbox" bind:checked={settings.sentinel_enabled} />
            <span>Enable Layer 6 Async AI Sentinel</span>
          </label>
          <label class="chk">
            <input type="checkbox" bind:checked={settings.auto_revert_rogue_payouts} />
            <span>Auto-revert unauthorized unprotected payouts</span>
          </label>
        </div>
      </section>

      <!-- DPDP PII Masking -->
      <section class="panel cfg-card">
        <h3><EyeOff size={15} color="#f59e0b" /> DPDP Act 2023 PII Masking</h3>
        <div class="field-grid">
          <label>
            <span>Token Format</span>
            <select class="select" bind:value={settings.dpdp_act.default_token_format}>
              <option value="AngleUuid">&lt;PII:TYPE:128-bit UUID&gt;</option>
              <option value="BracketIndexed">[TYPE_TOKEN_01]</option>
            </select>
          </label>
          <label>
            <span>Outbound PII Mode</span>
            <select class="select" bind:value={settings.dpdp_act.outbound_pii_mode}>
              <option value="MaskFull">Mask Full ([REDACTED:TYPE])</option>
              <option value="Last4">Last 4 Visible (XXXX-1234)</option>
              <option value="RehydrateAuthorized">Authorized Session Rehydrate</option>
            </select>
          </label>
        </div>
        <div class="checks two-col">
          <label class="chk"><input type="checkbox" bind:checked={settings.dpdp_act.mask_aadhaar} /><span>Aadhaar</span></label>
          <label class="chk"><input type="checkbox" bind:checked={settings.dpdp_act.mask_pan} /><span>PAN Card</span></label>
          <label class="chk"><input type="checkbox" bind:checked={settings.dpdp_act.mask_phone} /><span>Phone (+91)</span></label>
          <label class="chk"><input type="checkbox" bind:checked={settings.dpdp_act.mask_card} /><span>Card (Luhn)</span></label>
          <label class="chk"><input type="checkbox" bind:checked={settings.dpdp_act.mask_ifsc} /><span>IFSC Code</span></label>
          <label class="chk"><input type="checkbox" bind:checked={settings.dpdp_act.mask_upi} /><span>UPI VPA</span></label>
          <label class="chk"><input type="checkbox" bind:checked={settings.dpdp_act.mask_secrets} /><span>OTP / CVV / A/C</span></label>
          <label class="chk"><input type="checkbox" bind:checked={settings.dpdp_act.strict_aadhaar_verhoeff} /><span>Strict Verhoeff</span></label>
        </div>
      </section>

      <!-- Admin Perimeter Key -->
      <section class="panel cfg-card">
        <h3><KeyRound size={15} color="#a78bfa" /> Staff Perimeter Admin Lock</h3>
        <p class="muted" style="font-size: 12.5px;">
          When configured, an Admin Key is required to disable the Shield, change policies, or unmask customer PII in Staff Chats.
        </p>
        <label>
          <span>
            Admin Key Status:
            <b style="color: {settings.admin_key_configured ? '#10b981' : '#94a3b8'};">
              {settings.admin_key_configured ? 'LOCKED & ENFORCED' : 'Unset (Open Dev Mode)'}
            </b>
          </span>
          <input
            type="password"
            class="input mono"
            placeholder="Set or rotate Admin Key..."
            bind:value={adminKeyInput}
          />
        </label>
      </section>
    </div>
  {:else}
    <div class="panel" style="padding: 32px; text-align: center;">Loading configuration...</div>
  {/if}
</div>

<style>
  .settings-page {
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: 100%;
    overflow-y: auto;
  }
  .head-bar {
    padding: 15px 18px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .head-bar h2 {
    font-size: 15.5px;
    font-weight: 600;
  }
  .head-bar p {
    font-size: 12px;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .cfg-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
  }
  .cfg-card {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .cfg-card h3 {
    font-size: 13.5px;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }
  .field-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-1);
  }
  .checks {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .checks.two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .chk {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    color: var(--text-0);
  }
  .chk input {
    accent-color: var(--cyan);
  }
  .muted { color: var(--text-2); }
  @media (max-width: 980px) {
    .cfg-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
