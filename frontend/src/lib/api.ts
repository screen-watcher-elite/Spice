export interface LayerReport {
  layer: number;
  name: string;
  status: 'PASS' | 'MASKED' | 'BLOCKED' | 'HELD' | 'SKIPPED' | 'ALERT';
  latency_us: number;
  detail: string;
  metadata?: Record<string, unknown>;
}

export interface ToolTraceEntry {
  name: string;
  args: Record<string, unknown>;
  allowed: boolean;
  executed: boolean;
  violation_code?: string;
  reason: string;
  output?: Record<string, unknown>;
}

export interface TelemetryEvent {
  event_id: string;
  session_id?: string;
  account_id?: string;
  timestamp: number;
  iso_time: string;
  event_type: string;
  target_model: string;
  decision_source?: string;
  overhead_ms: number;
  user_prompt_sanitized: string;
  pii_masked: Record<string, number>;
  attack_type?: string;
  tool_name?: string;
  tool_args?: Record<string, unknown>;
  violation_code?: string;
  reason?: string;
  layers: LayerReport[];
}

export interface TelemetryStats {
  total_requests: number;
  blocked_attacks: number;
  pii_masked_count: number;
  circuit_trips: number;
  sentinel_interventions: number;
  staff_overrides: number;
  avg_overhead_ms: number;
  p50_overhead_ms: number;
  p95_overhead_ms: number;
  p99_overhead_ms: number;
  active_connections: number;
  recent_latencies_ms: number[];
}

export interface ShieldStatus {
  shield_active: boolean;
  policy_preset: string;
  decision_model: string;
  decisions_mode: string;
  agent_model: string;
  gemini_configured: boolean;
  admin_key_required: boolean;
  started_at: string;
  pending_overrides: number;
  metrics: TelemetryStats;
}

export interface CustomerSummary {
  customer_id: string;
  full_name: string;
  account_id: string;
  upi_vpa: string;
  balance: number;
  credit_limit: number;
  cibil_score: number;
  kyc_status: string;
  risk_tier: string;
  is_frozen: boolean;
  open_disputes_count: number;
}

export interface CustomerProfile extends CustomerSummary {
  email: string;
  phone: string;
  pan_number: string;
  aadhaar_number: string;
  date_of_birth: string;
  address: string;
  city: string;
  state: string;
  pincode: string;
  account_number: string;
  ifsc_code: string;
  available_credit: number;
}

export interface SessionRecord {
  session_id: string;
  account_id: string;
  customer_name: string;
  status: 'AI_ACTIVE' | 'NEEDS_OVERRIDE' | 'STAFF_TAKEOVER' | 'QUARANTINED' | 'CLOSED';
  assigned_officer?: string;
  risk_score: number;
  pending_overrides_count: number;
  message_count: number;
  last_message_preview?: string;
  created_at: string;
  updated_at: string;
}

export interface ChatMessageRecord {
  msg_id: string;
  session_id: string;
  role: 'customer' | 'assistant' | 'staff' | 'system';
  content_sanitized: string;
  content_unmasked?: string;
  pii_counts: Record<string, number>;
  layer_trace?: LayerReport[];
  tool_calls?: ToolTraceEntry[];
  decision_source?: string;
  sentinel_verdict?: string;
  overhead_ms: number;
  created_at: string;
}

export interface PendingOverrideRecord {
  override_id: string;
  session_id: string;
  account_id: string;
  tool_name: string;
  tool_args: Record<string, unknown>;
  violation_code: string;
  reason: string;
  status: 'PENDING' | 'APPROVED' | 'REJECTED' | 'MODIFIED_APPROVED';
  decided_by?: string;
  decision_notes?: string;
  execution_result?: Record<string, unknown>;
  created_at: string;
  decided_at?: string;
}

export interface SentinelAuditRecord {
  audit_id: string;
  session_id?: string;
  event_id?: string;
  engine: string;
  severity: string;
  drift_score: number;
  rbi_grade: string;
  verdict: string;
  intervention: string;
  summary: string;
  details: Record<string, unknown>;
  acknowledged: boolean;
  created_at: string;
}

export interface SentinelSummary {
  total_audited: number;
  interventions_triggered: number;
  active_alerts: number;
  alignment_drift_score: number;
  rbi_compliance_rating: string;
  dpdp_data_minimization_pct: number;
  last_audit_timestamp?: string;
  last_engine_used: string;
  recent_drift_trend: number[];
  recent_audits: SentinelAuditRecord[];
}

export interface PublicSettingsView {
  policy_preset: string;
  rate_limit_rpm: number;
  admin_key_configured: boolean;
  statutory_limits: {
    max_auto_refund_amount: number;
    max_24h_refund_total: number;
    prohibit_frozen_accounts: boolean;
    enforce_kyc_transition_rules: boolean;
    max_credit_limit_multiplier: number;
    allowed_tools: string[];
  };
  dpdp_act: {
    vault_ttl_seconds: number;
    default_token_format: 'AngleUuid' | 'BracketIndexed';
    strict_aadhaar_verhoeff: boolean;
    outbound_pii_mode: 'MaskFull' | 'Last4' | 'RehydrateAuthorized';
    mask_pan: boolean;
    mask_aadhaar: boolean;
    mask_phone: boolean;
    mask_email: boolean;
    mask_card: boolean;
    mask_ifsc: boolean;
    mask_upi: boolean;
    mask_secrets: boolean;
  };
  gemini_api_key_configured: boolean;
  gemini_api_key_masked?: string;
  agent_model: string;
  decision_model: string;
  decisions_endpoint: string;
  decisions_mode: 'Smart' | 'Strict' | 'Off';
  sentinel_enabled: boolean;
  sentinel_model: string;
  auto_quarantine_threshold: number;
  auto_revert_rogue_payouts: boolean;
}

let currentAdminKey = '';
if (typeof window !== 'undefined') {
  currentAdminKey = sessionStorage.getItem('aegismesh_admin_key') || '';
}

export function setAdminKey(key: string) {
  currentAdminKey = key.trim();
  if (typeof window !== 'undefined') {
    if (currentAdminKey) sessionStorage.setItem('aegismesh_admin_key', currentAdminKey);
    else sessionStorage.removeItem('aegismesh_admin_key');
  }
}

export function getAdminKey(): string {
  return currentAdminKey;
}

async function req<T>(url: string, init?: RequestInit): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...((init?.headers as Record<string, string>) || {}),
  };
  if (currentAdminKey) {
    headers['x-admin-key'] = currentAdminKey;
  }
  const resp = await fetch(url, { ...init, headers });
  const data = await resp.json().catch(() => ({}));
  if (!resp.ok) {
    throw new Error(data.error || `HTTP ${resp.status}`);
  }
  return data as T;
}

export const api = {
  verifyAdminKey: () =>
    req<{ valid: boolean; admin_key_required: boolean }>('/api/v1/auth/verify', {
      method: 'POST',
      body: '{}',
    }),
  getShieldStatus: () => req<ShieldStatus>('/v1/shield/status'),
  toggleShield: (active: boolean, officer: string) =>
    req<{ success: boolean; shield_active: boolean; message: string }>('/v1/shield/toggle', {
      method: 'POST',
      body: JSON.stringify({ active, officer }),
    }),
  runChatCompletion: (payload: {
    prompt: string;
    account_id: string;
    session_id?: string;
    persist_chat?: boolean;
  }) =>
    req<{
      event_id: string;
      session_id: string;
      status: string;
      shield_active: boolean;
      overhead_ms: number;
      pii_detected: boolean;
      pii_counts: Record<string, number>;
      sanitized_prompt: string;
      reply: string;
      model_used: string;
      decision_source?: string;
      violation_code?: string;
      reason?: string;
      layers: LayerReport[];
      tool_traces: ToolTraceEntry[];
      created_overrides: PendingOverrideRecord[];
    }>('/v1/chat/completions', {
      method: 'POST',
      body: JSON.stringify(payload),
    }),

  listCustomers: () => req<{ customers: CustomerSummary[] }>('/api/v1/customers'),
  getCustomerDetail: (id: string) =>
    req<{
      profile: CustomerProfile;
      transactions: Array<Record<string, unknown>>;
      disputes: Array<Record<string, unknown>>;
      audit_logs: Array<Record<string, unknown>>;
    }>(`/api/v1/customers/${encodeURIComponent(id)}`),
  manualStaffAction: (accountId: string, body: Record<string, unknown>) =>
    req<Record<string, unknown>>(
      `/api/v1/customers/${encodeURIComponent(accountId)}/manual-action`,
      { method: 'POST', body: JSON.stringify(body) },
    ),

  listSessions: (status?: string) =>
    req<{ sessions: SessionRecord[] }>(
      `/api/v1/sessions${status && status !== 'ALL' ? `?status=${encodeURIComponent(status)}` : ''}`,
    ),
  createSession: (account_id: string) =>
    req<{ session: SessionRecord }>('/api/v1/sessions', {
      method: 'POST',
      body: JSON.stringify({ account_id }),
    }),
  getSessionDetail: (id: string) =>
    req<{
      session: SessionRecord;
      messages: ChatMessageRecord[];
      overrides: PendingOverrideRecord[];
      customer?: CustomerProfile;
      transactions: Array<Record<string, unknown>>;
      disputes: Array<Record<string, unknown>>;
    }>(`/api/v1/sessions/${encodeURIComponent(id)}`),
  sendCustomerMessage: (sessionId: string, content: string) =>
    req<Record<string, unknown>>(`/api/v1/sessions/${encodeURIComponent(sessionId)}/message`, {
      method: 'POST',
      body: JSON.stringify({ content }),
    }),
  toggleTakeover: (sessionId: string, takeover: boolean, officer: string) =>
    req<{ success: boolean; status: string }>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/takeover`,
      { method: 'POST', body: JSON.stringify({ takeover, officer }) },
    ),
  sendStaffReply: (sessionId: string, officer: string, content: string) =>
    req<{ success: boolean; message: ChatMessageRecord }>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/staff-reply`,
      { method: 'POST', body: JSON.stringify({ officer, content }) },
    ),
  toggleQuarantine: (sessionId: string, quarantined: boolean, officer: string, reason: string) =>
    req<{ success: boolean; status: string }>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/quarantine`,
      { method: 'POST', body: JSON.stringify({ quarantined, officer, reason }) },
    ),
  revealPii: (sessionId: string, officer: string, reason: string) =>
    req<{ success: boolean; messages: ChatMessageRecord[] }>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/reveal-pii`,
      { method: 'POST', body: JSON.stringify({ officer, reason }) },
    ),
  purgeSessionPii: (sessionId: string, officer: string) =>
    req<{ success: boolean; purged_messages: number }>(
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/purge-pii`,
      { method: 'POST', body: JSON.stringify({ officer, reason: 'DPDP right-to-erasure purge' }) },
    ),

  listOverrides: (status?: string) =>
    req<{ overrides: PendingOverrideRecord[] }>(
      `/api/v1/overrides${status ? `?status=${encodeURIComponent(status)}` : ''}`,
    ),
  decideOverride: (
    overrideId: string,
    payload: {
      action: 'APPROVE' | 'REJECT' | 'MODIFY_APPROVE';
      officer: string;
      notes: string;
      modified_args?: Record<string, unknown>;
    },
  ) =>
    req<{ success: boolean; override: PendingOverrideRecord; execution_result: Record<string, unknown> }>(
      `/api/v1/overrides/${encodeURIComponent(overrideId)}/decision`,
      { method: 'POST', body: JSON.stringify(payload) },
    ),

  getTelemetryStats: () => req<TelemetryStats>('/api/v1/telemetry/stats'),
  getTelemetryHistory: () => req<{ history: TelemetryEvent[] }>('/api/v1/telemetry/history'),
  getSentinelState: () => req<SentinelSummary>('/api/v1/sentinel/state'),
  ackSentinelAlert: (id: string) =>
    req<{ success: boolean }>(`/api/v1/sentinel/alerts/${encodeURIComponent(id)}/ack`, {
      method: 'POST',
      body: '{}',
    }),
  getStaffLogs: () => req<{ logs: Array<Record<string, unknown>> }>('/api/v1/staff-logs'),

  getSettings: () => req<PublicSettingsView>('/api/v1/settings'),
  updateSettings: (patch: Record<string, unknown>) =>
    req<{ success: boolean; settings: PublicSettingsView }>('/api/v1/settings', {
      method: 'PUT',
      body: JSON.stringify(patch),
    }),
  applyPolicyPreset: (preset: 'RBI_DPDP_FINTECH_V1' | 'STRICT_BANKING') =>
    req<{ success: boolean }>('/api/v1/policy/preset', {
      method: 'POST',
      body: JSON.stringify({ preset }),
    }),
  downloadAuditReport: async (format: 'json' | 'csv') => {
    const headers: Record<string, string> = {};
    if (currentAdminKey) {
      headers['x-admin-key'] = currentAdminKey;
    }
    const resp = await fetch(`/api/v1/audit/export?format=${format}`, { headers });
    if (!resp.ok) {
      const data = await resp.json().catch(() => ({}));
      throw new Error(data.error || `Export failed (HTTP ${resp.status})`);
    }
    const blob = await resp.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `aegismesh_compliance_audit.${format}`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  },
};
