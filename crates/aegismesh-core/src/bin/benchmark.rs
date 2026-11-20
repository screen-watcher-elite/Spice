use std::time::Instant;

use aegismesh_core::config::{PiiConfig, PolicyConfig};
use aegismesh_core::layer1_regex::Layer1RegexEngine;
use aegismesh_core::layer2_pii::tokenizer::PiiTokenizer;
use aegismesh_core::layer2_pii::vault::PiiVault;
use aegismesh_core::layer4_breaker::{CircuitBreakerEvaluator, ToolCall};
use aegismesh_core::layer5_outbound::OutboundFirewall;
use serde_json::json;

fn main() {
    let iterations = 5000usize;
    let l1 = Layer1RegexEngine::new();
    let vault = PiiVault::new(300);
    let tok = PiiTokenizer::new(Some(vault));
    let pii_cfg = PiiConfig::default();
    let cb = CircuitBreakerEvaluator::new();
    let pol = PolicyConfig::rbi_dpdp_default();
    let out_fw = OutboundFirewall::new(tok.clone());

    let sample_prompt =
        "Hi, my PAN is FDCPC9935C and phone +91 98469 13810. Refund ₹450 for Swiggy double debit.";
    let sample_tool = ToolCall {
        name: "execute_refund".into(),
        args: Some(json!({"account_id": "MMX-AC-5001", "amount": 450.0})),
    };
    let sample_reply = "Your ₹450.00 refund for account MMX-AC-5001 has been approved.";

    // Warm up
    for _ in 0..100 {
        let _ = l1.inspect(sample_prompt);
        let _ = tok.tokenize_with_config(sample_prompt, "warm", &pii_cfg, None);
    }

    let mut latencies_us = Vec::with_capacity(iterations);
    let t_all = Instant::now();

    for i in 0..iterations {
        let t0 = Instant::now();
        let s1 = l1.inspect(sample_prompt);
        assert!(s1.is_safe);
        let sid = format!("bench_{}", i % 50);
        let s2 = tok.tokenize_with_config(sample_prompt, &sid, &pii_cfg, None);
        let s4 = cb.evaluate(&sample_tool, &pol, None, None, None);
        assert!(s4.allowed);
        let _s5 = out_fw.inspect_and_sanitize(sample_reply, &sid, &pii_cfg);
        let _ = s2.has_pii;
        latencies_us.push(t0.elapsed().as_nanos() as f64 / 1000.0);
    }

    let total_sec = t_all.elapsed().as_secs_f64();
    latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg: f64 = latencies_us.iter().sum::<f64>() / latencies_us.len() as f64;
    let p50 = latencies_us[latencies_us.len() / 2];
    let p95 = latencies_us[(latencies_us.len() * 95) / 100];
    let p99 = latencies_us[(latencies_us.len() * 99) / 100];
    let ops = iterations as f64 / total_sec;

    println!("\n==================================================================");
    println!("  AEGISMESH v2.0 INLINE SECURITY PIPELINE BENCHMARK (RUST RELEASE)");
    println!("==================================================================");
    println!("  Iterations : {}", iterations);
    println!("  Throughput : {:.0} ops/sec", ops);
    println!("  Avg Latency: {:.2} µs ({:.3} ms)", avg, avg / 1000.0);
    println!("  P50 Latency: {:.2} µs ({:.3} ms)", p50, p50 / 1000.0);
    println!("  P95 Latency: {:.2} µs ({:.3} ms)", p95, p95 / 1000.0);
    println!("  P99 Latency: {:.2} µs ({:.3} ms)", p99, p99 / 1000.0);
    println!("==================================================================\n");
}
