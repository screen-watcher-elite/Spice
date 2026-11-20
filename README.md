# AegisMesh v2.0 — Fintech Agentic AI Security Mesh

AegisMesh is a Rust + Svelte 5 security gateway, RBI & DPDP Act 2023 compliance firewall, and staff supervision console built for customer-facing fintech agentic AI deployments. It compiles into a single portable Windows executable (`aegismesh.exe`) with an embedded Svelte 5 UI and a pure-Rust Windows installer (`aegismesh-setup.exe`).

---

## 6-Layer Security Architecture

1. **Layer 1 — Adversarial Normalizer & Regex Firewall (`layer1_regex`)**
   - Normalizes Unicode NFKC, zero-width & BiDi control characters, Cyrillic/Greek homoglyphs, HTML/SQL comments, Markdown formatting, URL percent-encoding (`%XX`), `\uXXXX` escapes, multi-alphabet Base64 segments, leetspeak, spaced letters, and Hinglish attack phrases.
   - Enforces a 16,384-character DoS ceiling and a sharded token-bucket `RateLimiter` (`rate_limit_rpm`).

2. **Layer 2 — DPDP Act 2023 PII Tokenizer & Ephemeral Vault (`layer2_pii`)**
   - Single-pass $O(N)$ non-overlapping masking across PAN, Aadhaar (optional Verhoeff validation), Indian `+91` phone numbers, email addresses, payment cards (Luhn validation), IFSC, UPI VPA, bank accounts, OTP/CVV, and credentials.
   - Maps tokens inside an ephemeral `parking_lot::RwLock` `PiiVault` with 128-bit UUID keys and a 30-second Tokio expiry janitor.

3. **Layer 3 — AI Guardrails & Google Gemini Decisions API (`layer3_guardrail`)**
   - Integrates with Google Gemini Decisions API (`https://generativelanguage.googleapis.com/v1beta/decisions`, `gemini-decision-1`) under configurable `Smart`, `Strict`, or `Off` modes.
   - Backed by a SHA-256 prompt-decision LRU/TTL cache and deterministic local fallback (`GEMINI_CLASSIFIER_FALLBACK` / `OFFLINE_HEURISTIC`) when no API key is configured or offline.

4. **Layer 4 — Live DB-Backed RBI Statutory Circuit Breaker (`layer4_breaker`)**
   - Enforces the ₹5,000 single auto-refund cap and a `BEGIN IMMEDIATE` atomic 24-hour rolling smurfing velocity breaker (`max_24h_refund_total`).
   - Enforces live frozen-account blocks, full identifier IDOR verification across masked/last-4/raw identifiers, minimum refund validation (`>= ₹1.00`), `1.25×` credit limit cap, fail-closed KYC state transitions, and routes high-value or anomalous tool calls to the **Maker-Checker** human override queue.

5. **Layer 5 — Outbound Response Firewall (`layer5_outbound`)**
   - Scans assistant responses for system-prompt leakage and scrubs raw DB-sourced customer PII before egress, preserving only authorized session tokens in `RehydrateAuthorized` mode.

6. **Layer 6 — Asynchronous AI Sentinel (`layer6_sentinel`)**
   - Non-blocking Tokio background auditor scoring multi-turn customer/agent trajectories (via `gemini-3-flash-preview` or heuristic fallback), computing compliance grades (`AAA` / `AA` / `A` / `BBB`) and alignment drift scores, persisting `sentinel_audits`, streaming `SENTINEL_ALERT` SSE events, and executing active interventions (`HOLD_AND_ESCALATE`, `QUARANTINE_SESSION`, `AUTO_REVERT_PAYOUT`).

---

## Red-Team Hardening Highlights

- **Atomic Concurrency & Anti-Race Protection**: Transactional `claim_override_for_decision` (`Pending` → `Processing` → `Approved`/`Rejected`) and SQLite `BEGIN IMMEDIATE` smurfing velocity reservations prevent double-refund race conditions.
- **Zero Plaintext PII by Default**: `CustomerProfile::masked()` masks PAN, Aadhaar, account numbers, phones, and emails on public read endpoints; unmasked chat history requires Admin Key verification + staff audit logging via `POST /api/v1/sessions/:id/reveal-pii`, with DPDP erasure supported via `POST /api/v1/sessions/:id/purge-pii`.
- **Fail-Closed Perimeter & XSS Defense**:
  - Loopback-restricted CORS (`127.0.0.1` / `localhost`), CSP (`default-src 'self'; script-src 'self'; ...`), `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Cross-Origin-Resource-Policy: same-origin`, `Permissions-Policy`.
  - 256 KB body limit, 30s request timeout, constant-time SHA-256 Admin Key verification with 5-attempt / 60s `429` brute-force lockout.
  - CSV formula-injection neutralization on `/api/v1/audit/export` and sanitized `Turn 2` tool-response JSON inside `FinBotAgent`.

---

## Measured Verification & Benchmarks

Verified on Windows (`rustc 1.97.1`, `svelte-check`, Vite `v6.4.6`) via [`scripts/verify_all.ps1`](scripts/verify_all.ps1):

| Check | Result |
| --- | --- |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings, 0 errors |
| `cargo test --workspace` | 17/17 passed (`test_db` 4, `test_redteam_hardening` 4, `test_security_layers` 6, `test_api` 3) |
| `redteam_stress` (Release) | 20/20 passed; 500 concurrent async tasks in 6.34 ms (78,828 ops/sec), 0 cross-session leaks |
| `benchmark` (Release, 5,000 iterations) | 20,558 ops/sec · Avg 47.82 µs (0.048 ms) · P50 34.50 µs · P95 72.30 µs · P99 108.80 µs |
| `svelte-check` + Vite build | 0 errors, 0 warnings · 64.22 kB total gzipped bundle |

---

## Project Layout

```text
├── crates/
│   ├── aegismesh-core/     # 6-layer security engine, MoneyMax & WAL StateStore, FinBotAgent
│   ├── aegismesh-server/   # Axum REST + SSE server, embedded Svelte 5 UI, CLI runner
│   └── aegismesh-setup/    # Pure-Rust native Windows installer (embeds exe + seeded DB)
├── database/               # MoneyMax SQLite schema, seed script & seeded moneymax.db
├── frontend/               # Minimalistic Obsidian Svelte 5 SaaS console
└── scripts/
    ├── verify_all.ps1      # Clippy, workspace tests, stress suite, benchmark, Svelte check/build
    └── build_release.ps1   # Builds dist/aegismesh.exe, aegismesh-setup.exe, AegisMesh-Portable.zip
```

---

## Quick Start (Development & Portable `.exe`)

### Run Verification & Build Release Executables

```powershell
# Run full backend + stress + benchmark + frontend check
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify_all.ps1

# Build standalone dist\aegismesh.exe, dist\aegismesh-setup.exe, and dist\AegisMesh-Portable.zip
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build_release.ps1
```

### Run Standalone Executable (`aegismesh.exe`)

If `database\moneymax.db` is absent, `aegismesh.exe` automatically bootstraps a sample database alongside `database\aegismesh_state.db` and opens the console in the browser (port fallback up to +15 ports if `8080` is busy).

```powershell
# Interactive startup (auto-opens http://127.0.0.1:8080)
.\dist\aegismesh.exe

# Custom port / headless flags
.\dist\aegismesh.exe --host 127.0.0.1 --port 8080 --no-browser `
  --db-path .\database\moneymax.db `
  --state-db-path .\database\aegismesh_state.db `
  --admin-key "your-staff-secret"
```

### Silent Windows Installer (`aegismesh-setup.exe`)

```powershell
.\dist\aegismesh-setup.exe --silent --no-launch --dir "C:\Apps\AegisMesh"
# Optional flags: --force-db (overwrite existing moneymax.db), --no-shortcuts
```

### Configuration & 3-Model Gemini Architecture

Copy [`aegismesh.toml.example`](aegismesh.toml.example) to `aegismesh.local.toml` (gitignored), configure via local `.env` (gitignored), or configure live in **Settings → Google Gemini & Decisions API**:

- **Fintech Customer Agent (`GEMINI_AGENT_MODEL`)**: `gemini-3.5-flash-lite` (stable GA, low-cost tool-calling agent with failover to `gemini-3-flash-preview` / `gemini-3.1-flash-lite`).
- **Inline Layer 3 Guardrail (`GEMINI_DECISION_MODEL`)**: `gemini-decision-1` via `POST /v1beta/decisions` (zero-output-token calibrated `noul` + `choice` gate).
- **Async Layer 6 Sentinel (`GEMINI_SENTINEL_MODEL`)**: `gemini-3-flash-preview` (thinking-enabled non-blocking trajectory auditor off the customer hot path).

For Paid-tier deployments, you can upgrade `GEMINI_AGENT_MODEL` to `gemini-4-hydrogen` and `GEMINI_SENTINEL_MODEL` to `gemini-4-carbon` or `gemini-4-argon`. Without `GEMINI_API_KEY`, AegisMesh runs deterministically in Offline Simulator mode.

### Deploy to Google Cloud Run (Mumbai `asia-south1`)

AegisMesh includes production container & Google Cloud Run deployment configuration designed for Indian fintech DPDP data residency (`asia-south1` Mumbai), listening on `0.0.0.0:$PORT`, handling `SIGTERM` graceful shutdown, and loading keys from Secret Manager:

- [`Dockerfile`](Dockerfile): Multi-stage build (`node:22-slim` Svelte UI → `rust:1-bookworm` binary → `debian:bookworm-slim` non-root runtime with `/health` check).
- [`cloudbuild.yaml`](cloudbuild.yaml): Cloud Build pipeline pushing to Artifact Registry and deploying to managed Cloud Run.
- [`deploy/cloudrun-service.yaml`](deploy/cloudrun-service.yaml): Declarative Knative service spec with startup/liveness probes and Secret Manager refs.
- [`scripts/deploy_gcp.ps1`](scripts/deploy_gcp.ps1) & [`scripts/deploy_gcp.sh`](scripts/deploy_gcp.sh): One-command deployment (`.\scripts\deploy_gcp.ps1 -ProjectId <YOUR_GCP_PROJECT_ID>`).

---

## API Overview

| Group | Endpoints |
| --- | --- |
| **Health & Gateway** | `GET /health`, `GET /v1/shield/status`, `POST /v1/shield/toggle`, `POST /v1/shield/inspect`, `POST /v1/chat/completions` |
| **Customers & Ledger** | `GET /api/v1/customers`, `GET /api/v1/customers/:id`, `POST /api/v1/customers/:id/manual-action` |
| **Customer Chats & Maker-Checker** | `GET/POST /api/v1/sessions`, `GET /api/v1/sessions/:id`, `POST /api/v1/sessions/:id/message`, `POST /api/v1/sessions/:id/takeover`, `POST /api/v1/sessions/:id/staff-reply`, `POST /api/v1/sessions/:id/quarantine`, `POST /api/v1/sessions/:id/reveal-pii`, `POST /api/v1/sessions/:id/purge-pii`, `GET /api/v1/overrides`, `POST /api/v1/overrides/:id/decision` |
| **Telemetry & Sentinel** | `GET /api/v1/telemetry/stats`, `GET /api/v1/telemetry/history`, `GET /api/v1/telemetry/stream` (`/v1/telemetry/events`), `GET /api/v1/sentinel/state`, `POST /api/v1/sentinel/alerts/:id/ack`, `GET /api/v1/audit/export`, `GET /api/v1/staff-logs` |
| **Policy, Settings & Auth** | `POST /api/v1/auth/verify`, `GET/PUT /api/v1/policy`, `POST /api/v1/policy/preset`, `GET/PUT /api/v1/settings` |
