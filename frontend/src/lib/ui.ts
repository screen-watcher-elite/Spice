// Shared presentation helpers for the AegisMesh UI.

export type Tone = 'ok' | 'warn' | 'danger' | 'staff' | 'info' | 'mute';

export function toneForEvent(t: string | undefined): Tone {
  switch (t) {
    case 'BLOCKED':
    case 'UNPROTECTED_RUN':
    case 'SENTINEL_ALERT':
      return 'danger';
    case 'CIRCUIT_TRIP':
    case 'CIRCUIT_BREAKER_TRIPPED':
    case 'MASKED':
    case 'HELD':
    case 'ALERT':
      return 'warn';
    case 'STAFF_OVERRIDE':
      return 'staff';
    default:
      return 'ok';
  }
}

export function toneForLayer(status: string | undefined): Tone {
  switch (status) {
    case 'BLOCKED':
      return 'danger';
    case 'HELD':
    case 'MASKED':
    case 'ALERT':
      return 'warn';
    case 'SKIPPED':
      return 'mute';
    default:
      return 'ok';
  }
}

export function toneForSeverity(sev: string | undefined): Tone {
  switch (sev) {
    case 'CRITICAL':
    case 'HIGH':
      return 'danger';
    case 'MEDIUM':
      return 'warn';
    default:
      return 'ok';
  }
}

export function toneForSession(st: string): Tone {
  switch (st) {
    case 'NEEDS_OVERRIDE':
      return 'warn';
    case 'STAFF_TAKEOVER':
      return 'staff';
    case 'QUARANTINED':
      return 'danger';
    case 'CLOSED':
      return 'mute';
    default:
      return 'ok';
  }
}

export function friendlySession(st: string): string {
  switch (st) {
    case 'NEEDS_OVERRIDE':
      return 'Needs approval';
    case 'STAFF_TAKEOVER':
      return 'Staff live';
    case 'QUARANTINED':
      return 'Quarantined';
    case 'AI_ACTIVE':
      return 'AI handling';
    case 'CLOSED':
      return 'Closed';
    default:
      return st.replace(/_/g, ' ').toLowerCase();
  }
}

export function humanize(s: string | undefined): string {
  if (!s) return '';
  return s.replace(/_/g, ' ');
}

export function fmtUs(us?: number): string {
  if (us === undefined || us === null || !Number.isFinite(us)) return '—';
  if (us >= 1000) return `${(us / 1000).toFixed(2)}ms`;
  return `${Math.round(us)}µs`;
}

export function fmtMs(ms?: number, digits = 2): string {
  if (ms === undefined || ms === null || !Number.isFinite(ms)) return '—';
  return `${ms.toFixed(digits)}ms`;
}

export function fmtINR(n: unknown, frac = 0): string {
  const v = Number(n);
  if (!Number.isFinite(v)) return '₹—';
  return `₹${v.toLocaleString('en-IN', { minimumFractionDigits: frac, maximumFractionDigits: frac })}`;
}

export function fmtClock(iso: string | undefined): string {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

export function relTime(iso: string | undefined): string {
  if (!iso) return '';
  const d = new Date(iso).getTime();
  if (Number.isNaN(d)) return '';
  const s = Math.max(0, Math.round((Date.now() - d) / 1000));
  if (s < 45) return 'now';
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h`;
  return `${Math.round(h / 24)}d`;
}

export function initials(name: string | undefined): string {
  if (!name) return '··';
  const parts = name.trim().split(/\s+/);
  return ((parts[0]?.[0] ?? '') + (parts.length > 1 ? parts[parts.length - 1][0] : '')).toUpperCase();
}

export function hueFor(seed: string | undefined): number {
  let h = 0;
  const s = seed || '';
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) % 360;
  return h;
}

/* ---------- PII token segmentation (render-safe, no {@html}) ---------- */

export interface TextSegment {
  text: string;
  pii?: string; // short label if this segment is a vault token
  full?: string;
}

const PII_RE =
  /(<PII:[A-Z0-9_]+:[0-9a-fA-F-]+>|\[[A-Z0-9_]+_TOKEN_\d+\]|\[REDACTED:[A-Z0-9_]+\]|XXXX-\d{2,4})/g;

export function segmentPii(input: string | undefined | null): TextSegment[] {
  const text = input ?? '';
  if (!text) return [];
  const out: TextSegment[] = [];
  let last = 0;
  for (const m of text.matchAll(PII_RE)) {
    const idx = m.index ?? 0;
    if (idx > last) out.push({ text: text.slice(last, idx) });
    const tok = m[0];
    out.push({ text: tok, pii: piiLabel(tok), full: tok });
    last = idx + tok.length;
  }
  if (last < text.length) out.push({ text: text.slice(last) });
  return out;
}

function piiLabel(tok: string): string {
  let m = tok.match(/^<PII:([A-Z0-9_]+):([0-9a-fA-F]{4})/);
  if (m) return `${m[1]} · ${m[2].toLowerCase()}`;
  m = tok.match(/^\[([A-Z0-9_]+)_TOKEN_(\d+)\]$/);
  if (m) return `${m[1]} #${m[2]}`;
  m = tok.match(/^\[REDACTED:([A-Z0-9_]+)\]$/);
  if (m) return `${m[1]} redacted`;
  return tok;
}

export function piiSummary(counts: Record<string, number> | undefined): string {
  if (!counts) return '';
  return Object.entries(counts)
    .filter(([, v]) => v > 0)
    .map(([k, v]) => `${v}× ${k}`)
    .join(' · ');
}
