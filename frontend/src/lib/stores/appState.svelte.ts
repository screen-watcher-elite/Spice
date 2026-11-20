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
  showAdminPopover = $state<boolean>(false);
  focusSessionId = $state<string | null>(null);
  focusChatsFilter = $state<string | null>(null);
  revision = $state<number>(0);
  theme = $state<'dark' | 'light'>('dark');
  demoMode = $state<boolean>(false);

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

  constructor() {
    if (typeof window !== 'undefined') {
      const saved = localStorage.getItem('aegismesh_theme');
      if (saved === 'light' || saved === 'dark') {
        this.theme = saved;
      }
      this.applyThemeDom();
      this.demoMode = localStorage.getItem('aegismesh_demo') === 'true';
      this.applyDemoDom();
    }
  }

  applyThemeDom() {
    if (typeof document !== 'undefined') {
      document.documentElement.dataset.theme = this.theme;
      const meta = document.querySelector('meta[name="theme-color"]');
      if (meta) meta.setAttribute('content', this.theme === 'light' ? '#f3f4f7' : '#060607');
    }
  }

  toggleTheme() {
    this.theme = this.theme === 'dark' ? 'light' : 'dark';
    if (typeof window !== 'undefined') {
      localStorage.setItem('aegismesh_theme', this.theme);
    }
    this.applyThemeDom();
  }

  applyDemoDom() {
    if (typeof document !== 'undefined') {
      document.documentElement.dataset.demo = this.demoMode ? 'true' : 'false';
    }
  }

  toggleDemoMode() {
    this.demoMode = !this.demoMode;
    if (typeof window !== 'undefined') {
      localStorage.setItem('aegismesh_demo', String(this.demoMode));
    }
    this.applyDemoDom();
  }

  pushToast(kind: ToastItem['kind'], title: string, detail?: string) {
    const id = Math.random().toString(36).slice(2, 9);
    this.toasts = [{ id, kind, title, detail }, ...this.toasts].slice(0, 4);
    setTimeout(() => {
      this.toasts = this.toasts.filter((t) => t.id !== id);
    }, 4500);
  }

  openChatSession(sessionId: string) {
    this.focusSessionId = sessionId;
    this.focusChatsFilter = 'ALL';
    this.activeTab = 'chats';
  }

  openOverridesQueue() {
    this.focusSessionId = null;
    this.focusChatsFilter = 'NEEDS_OVERRIDE';
    this.activeTab = 'chats';
  }

  requestAdminUnlock(detail?: string) {
    this.showAdminPopover = true;
    this.pushToast(
      'error',
      'Admin key required',
      detail || 'Enter AEGIS_ADMIN_KEY in the topbar to unlock staff actions.',
    );
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
        this.revision += 1;
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
