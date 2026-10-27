//! Locate the user-mode `fltLib.lib` import library.
//!
//! `fltLib` is part of the Windows SDK/WDK user-mode libraries; it is not on
//! the default link search path used by every rustc invocation. This script
//! finds `um/x64/fltLib.lib` under Windows Kits and adds its directory.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CMFLT_SDK_LIB");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        println!("cargo:warning=cmflt-client is a Windows user-mode tool");
        return;
    }

    if let Some(dir) = find_sdk_lib() {
        println!("cargo:rustc-link-search=native={}", dir.display());
    } else {
        println!(
            "cargo:warning=fltLib.lib not found under Windows Kits; linking cmflt-client will fail. Set CMFLT_SDK_LIB to the .../um/x64 directory."
        );
    }
    println!("cargo:rustc-link-lib=static=fltLib");
}

fn find_sdk_lib() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("CMFLT_SDK_LIB") {
        let path = PathBuf::from(explicit);
        if path.join("fltLib.lib").exists() || path.join("FltLib.lib").exists() {
            return Some(path);
        }
    }

    let program_files_x86 =
        env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".to_string());
    let lib_root = Path::new(&program_files_x86)
        .join("Windows Kits")
        .join("10")
        .join("Lib");

    let mut versions: Vec<(Vec<u64>, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&lib_root).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(key) = version_key(&name) {
            let candidate = entry.path().join("um").join("x64");
            if candidate.join("fltLib.lib").exists() || candidate.join("FltLib.lib").exists() {
                versions.push((key, candidate));
            }
        }
    }
    versions.sort_by(|a, b| a.0.cmp(&b.0));
    versions.pop().map(|(_, path)| path)
}

fn version_key(name: &str) -> Option<Vec<u64>> {
    let mut parts = Vec::new();
    for part in name.split('.') {
        parts.push(part.parse::<u64>().ok()?);
    }
    if parts.len() >= 2 {
        Some(parts)
    } else {
        None
    }
}
