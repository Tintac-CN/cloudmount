//! Kernel link setup for the CloudMount minifilter.
//!
//! Responsibilities:
//! 1. locate the kernel-mode WDK import libraries;
//! 2. emit the MSVC link line for a native (NATIVE subsystem) driver image;
//! 3. fail closed if the architecture is not x86_64.
//!
//! `cargo check` does not link, so the driver crate remains type-checkable on
//! a machine without the kernel WDK; `scripts/build-driver.ps1` is the
//! supported build path and verifies the resulting PE.

use std::env;
use std::path::{Path, PathBuf};

const REQUIRED_LIBS: [&str; 5] = [
    "fltMgr",
    "ntoskrnl",
    "hal",
    "bufferoverflowfastfailk",
    "wmilib",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CMFLT_WDK_LIB");
    println!("cargo:rerun-if-env-changed=CMFLT_WDK_VERSION");
    println!("cargo:rerun-if-env-changed=ProgramFiles(x86)");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_os != "windows" || target_arch != "x86_64" {
        println!(
            "cargo:warning=cmflt-driver supports only x86_64-pc-windows-msvc (got {target_arch}-{target_os})"
        );
        return;
    }

    match find_wdk_lib_dir() {
        Some(lib_dir) => {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            for lib in REQUIRED_LIBS {
                println!("cargo:rustc-link-lib=static={lib}");
            }
        }
        None => {
            println!(
                "cargo:warning=kernel WDK import libraries not found; `cargo check` works but linking cloudmount.sys will fail. Install Microsoft.WindowsWDK.10.0.19041 or set CMFLT_WDK_LIB to the .../km/x64 directory."
            );
        }
    }

    // Kernel image link line. `/NODEFAULTLIB` is required: the crate is
    // `no_std` and must not pull in a user-mode CRT.
    for arg in [
        "/DRIVER",
        "/SUBSYSTEM:NATIVE",
        "/ENTRY:DriverEntry",
        "/NODEFAULTLIB",
        "/IGNORE:4257,4216",
        "/MACHINE:X64",
        "/NXCOMPAT",
        "/DYNAMICBASE",
    ] {
        println!("cargo:rustc-link-arg={arg}");
    }
}

fn find_wdk_lib_dir() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("CMFLT_WDK_LIB") {
        let path = PathBuf::from(explicit);
        if path.join("fltMgr.lib").exists() || path.join("FltMgr.lib").exists() {
            return Some(path);
        }
    }

    let requested = env::var("CMFLT_WDK_VERSION").ok();
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
            let candidate = entry.path().join("km").join("x64");
            if candidate.join("fltMgr.lib").exists() || candidate.join("FltMgr.lib").exists() {
                versions.push((key, candidate));
            }
        }
    }
    versions.sort_by(|a, b| a.0.cmp(&b.0));

    if let Some(requested) = requested {
        if let Some((_, path)) = versions
            .iter()
            .find(|(_, path)| path.to_string_lossy().contains(&requested))
        {
            return Some(path.clone());
        }
    }

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
