use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;

const BUNDLED_EXE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bundled_aegismesh.exe"));
const BUNDLED_DB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bundled_moneymax.db"));
const CONFIG_TEMPLATE: &str = include_str!("../../../aegismesh.toml.example");

#[derive(Parser, Debug)]
#[command(name = "aegismesh-setup", version, about = "AegisMesh Native Windows Installer")]
struct SetupArgs {
    #[arg(long, help = "Custom installation directory")]
    dir: Option<PathBuf>,
    #[arg(long, help = "Non-interactive installation")]
    silent: bool,
    #[arg(long, help = "Overwrite existing moneymax.db if present")]
    force_db: bool,
    #[arg(long, help = "Do not launch AegisMesh after installation")]
    no_launch: bool,
    #[arg(long, help = "Skip Desktop and Start Menu shortcut creation")]
    no_shortcuts: bool,
}

fn default_install_dir() -> PathBuf {
    if let Ok(local) = env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("AegisMesh")
    } else {
        PathBuf::from(r"C:\AegisMesh")
    }
}

fn create_windows_shortcuts(exe_path: &Path, install_dir: &Path) {
    #[cfg(target_os = "windows")]
    {
        let exe_str = exe_path.display().to_string().replace('\'', "''");
        let dir_str = install_dir.display().to_string().replace('\'', "''");
        let ps_script = format!(
            "$ws = New-Object -ComObject WScript.Shell; \
             $desk = [Environment]::GetFolderPath('Desktop'); \
             $s1 = $ws.CreateShortcut(\"$desk\\AegisMesh.lnk\"); \
             $s1.TargetPath = '{exe_str}'; \
             $s1.WorkingDirectory = '{dir_str}'; \
             $s1.Description = 'AegisMesh Fintech AI Firewall & Compliance SaaS'; \
             $s1.Save(); \
             $start = Join-Path ([Environment]::GetFolderPath('Programs')) 'AegisMesh'; \
             New-Item -ItemType Directory -Force -Path $start | Out-Null; \
             $s2 = $ws.CreateShortcut(\"$start\\AegisMesh.lnk\"); \
             $s2.TargetPath = '{exe_str}'; \
             $s2.WorkingDirectory = '{dir_str}'; \
             $s2.Save();"
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
            .output();
    }
}

fn main() {
    let args = SetupArgs::parse();
    let install_dir = args.dir.unwrap_or_else(default_install_dir);

    println!("==================================================================");
    println!("  AEGISMESH INSTALLER v{}", env!("CARGO_PKG_VERSION"));
    println!("==================================================================");
    println!("  Install Directory : {}", install_dir.display());
    println!("  Payload Mode      : Single-binary SaaS Gateway + Embedded UI");
    println!();

    if !args.silent {
        print!("Press ENTER to install AegisMesh (or Ctrl+C to cancel): ");
        let _ = io::stdout().flush();
        let mut buf = String::new();
        let _ = io::stdin().read_line(&mut buf);
    }

    if BUNDLED_EXE.len() < 1024 {
        eprintln!("Warning: installer was built without AEGISMESH_BUNDLE_EXE; writing placeholder.");
    }

    let db_dir = install_dir.join("database");
    fs::create_dir_all(&db_dir).expect("Failed to create installation directory");

    let exe_dest = install_dir.join("aegismesh.exe");
    fs::write(&exe_dest, BUNDLED_EXE).expect("Failed to write aegismesh.exe");

    let db_dest = db_dir.join("moneymax.db");
    if (!db_dest.exists() || args.force_db) && !BUNDLED_DB.is_empty() {
        fs::write(&db_dest, BUNDLED_DB).expect("Failed to write moneymax.db");
    }

    let cfg_dest = install_dir.join("aegismesh.toml");
    if !cfg_dest.exists() {
        fs::write(&cfg_dest, CONFIG_TEMPLATE).expect("Failed to write aegismesh.toml");
    }

    let uninstall_dest = install_dir.join("uninstall.bat");
    let uninstall_script = format!(
        "@echo off\r\n\
         del /Q \"%USERPROFILE%\\Desktop\\AegisMesh.lnk\" 2>nul\r\n\
         rmdir /S /Q \"%APPDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\AegisMesh\" 2>nul\r\n\
         echo Removed shortcuts. You may delete \"{}\" manually.\r\n\
         pause\r\n",
        install_dir.display()
    );
    let _ = fs::write(uninstall_dest, uninstall_script);

    println!("  [OK] Installed {}", exe_dest.display());
    println!("  [OK] Seeded    {}", db_dest.display());
    if !args.no_shortcuts {
        create_windows_shortcuts(&exe_dest, &install_dir);
        println!("  [OK] Created Desktop & Start Menu shortcuts");
    }
    println!();

    if !args.no_launch && BUNDLED_EXE.len() > 1024 {
        println!("Launching AegisMesh SaaS Console...");
        let _ = Command::new(&exe_dest).current_dir(&install_dir).spawn();
    }
}
