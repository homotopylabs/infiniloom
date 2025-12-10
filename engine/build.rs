//! Build script for infiniloom-engine
//!
//! When the `zig-core` feature is enabled, this script:
//! 1. Builds the Zig core library
//! 2. Links it to the Rust binary

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Only build Zig when zig-core feature is enabled
    if env::var("CARGO_FEATURE_ZIG_CORE").is_ok() {
        build_zig_core();
    }
}

fn build_zig_core() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let core_dir = PathBuf::from(&manifest_dir).parent().unwrap().join("core");
    let out_dir = env::var("OUT_DIR").unwrap();

    // Check if Zig is available
    let zig_check = Command::new("zig").arg("version").output();
    if zig_check.is_err() {
        panic!(
            "Zig compiler not found! Install Zig 0.13+ to use zig-core feature.\n\
             Install: brew install zig (macOS) or see https://ziglang.org/download/"
        );
    }

    println!("cargo:rerun-if-changed={}", core_dir.join("src").display());
    println!("cargo:rerun-if-changed={}", core_dir.join("build.zig").display());

    // Determine optimization level
    let profile = env::var("PROFILE").unwrap();
    let optimize = match profile.as_str() {
        "release" => "ReleaseFast",
        _ => "Debug",
    };

    // Build the Zig library
    let status = Command::new("zig")
        .current_dir(&core_dir)
        .args([
            "build",
            &format!("-Doptimize={}", optimize),
            "-p",
            &out_dir,
        ])
        .status()
        .expect("Failed to run zig build");

    if !status.success() {
        panic!("Zig build failed with status: {}", status);
    }

    // Link the library
    let lib_dir = PathBuf::from(&out_dir).join("lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=infiniloom-core");

    // On macOS/Linux, we need libc
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=System");

    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=c");
}
