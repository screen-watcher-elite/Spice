use std::collections::HashMap;
use serde_json::json;
use aegismesh_core::db::state_store::SessionStatus;
use aegismesh_core::pipeline::AegisMeshEngine;

/// Seeds realistic initial customer chats & Maker-Checker items on fresh database boot
/// so compliance staff immediately see live override & chat inspection capabilities.
pub fn seed_initial_sessions_if_empty(engine: &AegisMeshEngine) {
    let existing = engine.state_store.list_sessions(None).unwrap_or_default();
    if !existing.is_empty() {
        return;
    }

    // Session 1: Kavita Roy (MMX-AC-5002) - ₹25,000 Croma dispute held for Maker-Checker Staff Override
    let _ = engine
        .state_store
        .create_or_get_session("SESS-MMX-5002-01", "MMX-AC-5002", "Kavita Roy");
    let mut pii1 = HashMap::new();
    pii1.insert("PHONE".to_string(), 1);
    let _ = engine.state_store.append_message(
        "MSG-SEED-101",
        "SESS-MMX-5002-01",
        "customer",
        "Hi, I noticed an unauthorized ₹25,000 charge at Croma Electronics (TXN-MMX-100020). My registered mobile is <PII:PHONE:seed9791756179>. Please refund immediately.",
        "Hi, I noticed an unauthorized ₹25,000 charge at Croma Electronics (TXN-MMX-100020). My registered mobile is +919791756179. Please refund immediately.",
        &pii1,
        None,
        None,
        None,
        None,
        0.0,
    );
    let trace1 = json!([
        {"layer": 1, "name": "Regex & Adversarial Normalizer", "status": "PASS", "latency_us": 42, "detail": "No injection signatures detected."},
        {"layer": 2, "name": "DPDP PII Tokenizer", "status": "MASKED", "latency_us": 68, "detail": "Masked 1 PHONE into 128-bit UUID vault."},
        {"layer": 3, "name": "Gemini Decisions AI Guardrail", "status": "PASS", "latency_us": 190, "detail": "Legitimate customer dispute inquiry."},
        {"layer": 4, "name": "RBI Statutory Circuit Breaker", "status": "HELD", "latency_us": 115, "detail": "Refund ₹25,000.00 exceeds autonomous ₹5,000.00 RBI cap; escalated to staff."},
        {"layer": 5, "name": "Outbound Response Firewall", "status": "PASS", "latency_us": 31, "detail": "Outbound reply verified clean."},
        {"layer": 6, "name": "Async AI Sentinel", "status": "PASS", "latency_us": 4, "detail": "Queued for trajectory audit."}
    ]);
    let tools1 = json!([{
        "name": "execute_refund",
        "args": {"account_id": "MMX-AC-5002", "txn_id": "TXN-MMX-100020", "amount": 25000.0, "reason": "Disputed Croma Electronics chargeback"},
        "allowed": false,
        "executed": false,
        "violation_code": "ERR_REFUND_LIMIT_EXCEEDED",
        "reason": "RBI Fair Practices Code hold: Refund ₹25,000.00 exceeds autonomous ₹5,000.00 cap. Escalated for Maker-Checker staff approval."
    }]);
    let _ = engine.state_store.append_message(
        "MSG-SEED-102",
        "SESS-MMX-5002-01",
        "assistant",
        "I located dispute DSP-2026-0002 for ₹25,000.00 at Croma Electronics. Because this exceeds the ₹5,000 autonomous limit under RBI Fair Practices guidelines, I have escalated it to a MoneyMax Compliance Officer for Maker-Checker approval.",
        "I located dispute DSP-2026-0002 for ₹25,000.00 at Croma Electronics. Because this exceeds the ₹5,000 autonomous limit under RBI Fair Practices guidelines, I have escalated it to a MoneyMax Compliance Officer for Maker-Checker approval.",
        &HashMap::new(),
        Some(&trace1),
        Some(&tools1),
        Some("GEMINI_DECISIONS_API"),
        Some("CIRCUIT_TRIP"),
        0.45,
    );
    let _ = engine.state_store.create_pending_override(
        "OV-SEED-5002",
        "SESS-MMX-5002-01",
        "MMX-AC-5002",
        "execute_refund",
        &json!({
            "account_id": "MMX-AC-5002",
            "txn_id": "TXN-MMX-100020",
            "amount": 25000.0,
            "reason": "Disputed Croma Electronics chargeback (DSP-2026-0002)"
        }),
        "ERR_REFUND_LIMIT_EXCEEDED",
        "Refund ₹25,000.00 exceeds autonomous ₹5,000.00 RBI cap — awaiting staff Maker-Checker review.",
    );
    let _ = engine.state_store.update_session_status(
        "SESS-MMX-5002-01",
        SessionStatus::NeedsOverride,
        None,
        Some(0.45),
    );

    // Session 2: Sneha Chikane (MMX-AC-5001) - Clean ₹450 Swiggy inquiry
    let _ = engine
        .state_store
        .create_or_get_session("SESS-MMX-5001-01", "MMX-AC-5001", "Sneha Chikane");
    let mut pii2 = HashMap::new();
    pii2.insert("PAN".to_string(), 1);
    let _ = engine.state_store.append_message(
        "MSG-SEED-201",
        "SESS-MMX-5001-01",
        "customer",
        "Hello, my PAN is <PII:PAN:seedfdcpc9935c>. Could you check my open Swiggy dispute?",
        "Hello, my PAN is FDCPC9935C. Could you check my open Swiggy dispute?",
        &pii2,
        None,
        None,
        None,
        None,
        0.0,
    );
    let tools2 = json!([{
        "name": "get_disputes",
        "args": {"account_id": "MMX-AC-5001"},
        "allowed": true,
        "executed": true,
        "reason": "Tool call verified against live RBI & DPDP policy."
    }]);
    let _ = engine.state_store.append_message(
        "MSG-SEED-202",
        "SESS-MMX-5001-01",
        "assistant",
        "Your account MMX-AC-5001 has 1 open dispute (DSP-2026-0001) for ₹450.00 on Swiggy Delivery (TXN-MMX-100010), which is eligible for instant autonomous refund.",
        "Your account MMX-AC-5001 has 1 open dispute (DSP-2026-0001) for ₹450.00 on Swiggy Delivery (TXN-MMX-100010), which is eligible for instant autonomous refund.",
        &HashMap::new(),
        Some(&json!([
            {"layer": 1, "name": "Regex & Adversarial Normalizer", "status": "PASS", "latency_us": 36, "detail": "Cleared."},
            {"layer": 2, "name": "DPDP PII Tokenizer", "status": "MASKED", "latency_us": 55, "detail": "Masked 1 PAN token."},
            {"layer": 3, "name": "Gemini Decisions AI Guardrail", "status": "PASS", "latency_us": 30, "detail": "Smart fast-path."},
            {"layer": 4, "name": "RBI Statutory Circuit Breaker", "status": "PASS", "latency_us": 78, "detail": "1 read tool verified."},
            {"layer": 5, "name": "Outbound Response Firewall", "status": "PASS", "latency_us": 25, "detail": "Clean."},
            {"layer": 6, "name": "Async AI Sentinel", "status": "PASS", "latency_us": 4, "detail": "Compliant."}
        ])),
        Some(&tools2),
        Some("SMART_FAST_PATH"),
        Some("PASS"),
        0.23,
    );
}
