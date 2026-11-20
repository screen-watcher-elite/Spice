use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=AEGISMESH_BUNDLE_EXE");
    println!("cargo:rerun-if-env-changed=AEGISMESH_BUNDLE_DB");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));
    let dest_exe = out_dir.join("bundled_aegismesh.exe");
    let dest_db = out_dir.join("bundled_moneymax.db");

    if let Ok(exe_path) = env::var("AEGISMESH_BUNDLE_EXE") {
        let p = PathBuf::from(&exe_path);
        println!("cargo:rerun-if-changed={}", p.display());
        if p.exists() {
            fs::copy(&p, &dest_exe).expect("Failed to copy AEGISMESH_BUNDLE_EXE into OUT_DIR");
        } else {
            fs::write(&dest_exe, b"STUB").unwrap();
        }
    } else {
        fs::write(&dest_exe, b"STUB").unwrap();
    }

    let db_src = env::var("AEGISMESH_BUNDLE_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
                .join("..")
                .join("..")
                .join("database")
                .join("moneymax.db")
        });

    println!("cargo:rerun-if-changed={}", db_src.display());
    if db_src.exists() {
        fs::copy(&db_src, &dest_db).expect("Failed to copy moneymax.db into OUT_DIR");
    } else {
        fs::write(&dest_db, b"").unwrap();
    }
}
