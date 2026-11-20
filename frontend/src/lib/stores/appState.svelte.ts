import { api, type SentinelSummary, type ShieldStatus, type TelemetryEvent, type TelemetryStats } from '../api';

export type NavTab = 'overview' | 'chats' | 'sandbox' | 'sentinel' | 'settings';

export interface ToastItem {
  id: string;
  kind: 'info' | 'success' | 'warn' | 'error';
  title: string;
  detail?: string;
}

class AppStore {
  activeTab = $state<NavTab>('overview');
  officer = $state<string>('Officer Ashutosh');
  sseConnected = $state<boolean>(false);
  adminVerified = $state<boolean>(false);
  quickStartDismissed = $state<boolean>(false);

  shieldStatus = $state<ShieldStatus | null>(null);
  stats = $state<TelemetryStats>({
    total_requests: 0,
    blocked_attacks: 0,
    pii_masked_count: 0,
    circuit_trips: 0,
    sentinel_interventions: 0,
    staff_overrides: 0,
    avg_overhead_ms: 0,
    p50_overhead_ms: 0,
    p95_overhead_ms: 0,
    p99_overhead_ms: 0,
    active_connections: 0,
    recent_latencies_ms: [],
  });
  events = $state<TelemetryEvent[]>([]);
  sentinel = $state<SentinelSummary | null>(null);
  pendingOverridesCount = $state<number>(0);
  toasts = $state<ToastItem[]>([]);
  lastLayers = $state<TelemetryEvent['layers']>([]);

  private es: EventSource | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  pushToast(kind: ToastItem['kind'], title: string, detail?: string) {
    const id = Math.random().toString(36).slice(2, 9);
    this.toasts = [{ id, kind, title, detail }, ...this.toasts].slice(0, 4);
    setTimeout(() => {
      this.toasts = this.toasts.filter((t) => t.id !== id);
    }, 4500);
  }

  async refreshAll() {
    try {
      const [status, hist, sent] = await Promise.all([
        api.getShieldStatus(),
        api.getTelemetryHistory(),
        api.getSentinelState(),
      ]);
      this.shieldStatus = status;
      this.stats = status.metrics;
      this.pendingOverridesCount = status.pending_overrides;
      this.events = hist.history || [];
      if (this.events.length > 0 && this.events[0].layers?.length) {
        this.lastLayers = this.events[0].layers;
      }
      this.sentinel = sent;
    } catch (err) {
      console.error('Failed initial refresh:', err);
    }
  }

  connectSse() {
    if (this.es) {
      this.es.close();
    }
    const es = new EventSource('/api/v1/telemetry/stream');
    this.es = es;

    es.onopen = () => {
      this.sseConnected = true;
    };

    es.onmessage = (msg) => {
      try {
        const ev = JSON.parse(msg.data) as TelemetryEvent;
        this.events = [ev, ...this.events.filter((x) => x.event_id !== ev.event_id)].slice(0, 120);
        if (ev.layers && ev.layers.length > 0) {
          this.lastLayers = ev.layers;
        }
        // Refresh compact counters cleanly
        api.getShieldStatus().then((st) => {
          this.shieldStatus = st;
          this.stats = st.metrics;
          this.pendingOverridesCount = st.pending_overrides;
        }).catch(() => {});
        if (ev.event_type === 'SENTINEL_ALERT') {
          this.pushToast('error', 'Async Sentinel Intervention', ev.reason || ev.attack_type);
          api.getSentinelState().then((s) => (this.sentinel = s)).catch(() => {});
        } else if (ev.event_type === 'CIRCUIT_TRIP') {
          this.pushToast('warn', 'Maker-Checker Escalation', ev.reason);
        }
      } catch {
        // ignore keepalive parse noise
      }
    };

    es.onerror = () => {
      this.sseConnected = false;
      es.close();
      if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
      this.reconnectTimer = setTimeout(() => this.connectSse(), 2500);
    };
  }

  disconnectSse() {
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    if (this.es) {
      this.es.close();
      this.es = null;
    }
  }
}

export const appState = new AppStore();
