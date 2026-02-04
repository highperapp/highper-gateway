// Build script to provide build-time environment variables

use std::env;
use std::process::Command;

fn main() {
    // Get rustc version
    let rustc_version = Command::new("rustc")
        .arg("--version")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=RUSTC_VERSION={}", rustc_version);

    // Get target triple
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=TARGET={}", target);

    // Get build profile
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=PROFILE={}", profile);

    // Get build timestamp
    let build_date = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);

    // Rebuild if build script changes
    println!("cargo:rerun-if-changed=build.rs");
}
