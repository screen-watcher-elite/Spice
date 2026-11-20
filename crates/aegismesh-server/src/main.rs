use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use aegismesh_core::config::{AegisConfig, ConfigStore};
use aegismesh_core::db::moneymax::MoneyMaxDb;
use aegismesh_core::db::state_store::StateStore;
use aegismesh_core::pipeline::AegisMeshEngine;

use aegismesh_server::api::create_router;
use aegismesh_server::seed_sessions::seed_initial_sessions_if_empty;

#[derive(Parser, Debug)]
#[command(name = "aegismesh", version, about = "AegisMesh Fintech AI Firewall & Compliance SaaS")]
struct Cli {
    #[arg(long, help = "Host address to bind (default from config or 127.0.0.1)")]
    host: Option<String>,
    #[arg(long, help = "Port to bind (default from config or 8080)")]
    port: Option<u16>,
    #[arg(long, help = "Path to core MoneyMax SQLite database")]
    db_path: Option<PathBuf>,
    #[arg(long, help = "Path to AegisMesh operational state SQLite database")]
    state_db_path: Option<PathBuf>,
    #[arg(long, help = "Do not auto-launch web browser on startup")]
    no_browser: bool,
    #[arg(long, help = "Override Admin Key for this server session")]
    admin_key: Option<String>,
}

fn resolve_base_dir() -> PathBuf {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("database").exists() || cwd.join("aegismesh.toml").exists() {
        return cwd;
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            if parent.join("database").exists() || parent.join("aegismesh.toml").exists() {
                return parent.to_path_buf();
            }
            // In target/release during dev, check workspace root if nearby
            return parent.to_path_buf();
        }
    }
    cwd
}

fn resolve_db_paths(
    base_dir: &Path,
    db_override: Option<PathBuf>,
    state_override: Option<PathBuf>,
) -> (PathBuf, PathBuf) {
    let mm_path = db_override.unwrap_or_else(|| base_dir.join("database").join("moneymax.db"));
    let st_path = state_override.unwrap_or_else(|| base_dir.join("database").join("aegismesh_state.db"));
    (mm_path, st_path)
}

async fn bind_with_fallback(host: &str, preferred_port: u16) -> std::io::Result<(TcpListener, SocketAddr)> {
    for offset in 0..15 {
        let port = preferred_port.saturating_add(offset);
        let addr_str = format!("{}:{}", host, port);
        if let Ok(listener) = TcpListener::bind(&addr_str).await {
            let local = listener.local_addr()?;
            return Ok((listener, local));
        }
    }
    let fallback = format!("{}:0", host);
    let listener = TcpListener::bind(&fallback).await?;
    let local = listener.local_addr()?;
    Ok((listener, local))
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let base_dir = resolve_base_dir();
    let mut cfg = AegisConfig::load_from_dir(&base_dir);
    if let Some(ref k) = cli.admin_key {
        if !k.trim().is_empty() {
            cfg.firewall.admin_key = Some(k.trim().to_string());
        }
    }

    let (mm_path, st_path) = resolve_db_paths(&base_dir, cli.db_path, cli.state_db_path);
    let moneymax_db = MoneyMaxDb::open_or_bootstrap(&mm_path)
        .expect("Failed to open or bootstrap MoneyMax SQLite database");
    let state_store = StateStore::open(&st_path)
        .expect("Failed to open AegisMesh operational state database");

    // Overlay any saved UI runtime settings from state_store
    if let Ok(Some(saved_val)) = state_store.load_kv_setting("aegis_config") {
        if let Ok(mut saved_cfg) = serde_json::from_value::<AegisConfig>(saved_val) {
            // Preserve live env key if saved key is unset
            if saved_cfg.google_ai.api_key.is_none() {
                saved_cfg.google_ai.api_key = cfg.google_ai.api_key.clone();
            }
            if saved_cfg.firewall.admin_key.is_none() {
                saved_cfg.firewall.admin_key = cfg.firewall.admin_key.clone();
            }
            cfg = saved_cfg;
        }
    }

    let host = cli.host.unwrap_or_else(|| cfg.server.host.clone());
    let preferred_port = cli.port.unwrap_or(cfg.server.port);
    let auto_open = !cli.no_browser && cfg.server.auto_open_browser;

    let config_store = ConfigStore::new(cfg.clone());
    let engine = Arc::new(AegisMeshEngine::new(config_store, moneymax_db, state_store));

    seed_initial_sessions_if_empty(&engine);

    let (listener, local_addr) = bind_with_fallback(&host, preferred_port)
        .await
        .expect("Failed to bind server listener");

    let display_host = if local_addr.ip().is_unspecified() {
        "127.0.0.1".to_string()
    } else {
        local_addr.ip().to_string()
    };
    let ui_url = format!("http://{}:{}", display_host, local_addr.port());

    println!();
    println!("==================================================================");
    println!("  AEGISMESH v{} // FINTECH AI FIREWALL & COMPLIANCE SAAS", env!("CARGO_PKG_VERSION"));
    println!("==================================================================");
    println!("  SaaS Console : {}", ui_url);
    println!("  Core DB      : {}", mm_path.display());
    println!("  State DB     : {}", st_path.display());
    println!("  Shield State : {}", if engine.is_shield_active() { "ACTIVE (6-Layer)" } else { "BYPASSED" });
    println!(
        "  Gemini AI    : {}",
        if cfg.google_ai.has_live_key() {
            format!("LIVE ({})", cfg.google_ai.agent_model)
        } else {
            "OFFLINE / SIMULATOR MODE (configure key in Settings)".to_string()
        }
    );
    println!("==================================================================");
    println!("  Press Ctrl+C to shut down cleanly.");
    println!();

    if auto_open {
        open_browser(&ui_url);
    }

    let app = create_router(engine);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            println!("\nShutting down AegisMesh cleanly...");
        })
        .await
        .expect("Server encountered fatal error");
}
