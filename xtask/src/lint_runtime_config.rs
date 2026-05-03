//! CI lint enforcing the `runtime_config` discipline (per ROADMAP §0.1).
//!
//! Stage 1 scope: `highper-gateway/src/plugin/`. Stage 2 expands to
//! `cluster`, `cache`, `ai`. Stage 3 goes project-wide. See
//! `docs/planning/RUNTIME_CONFIG_STAGE1_PR_PLAN.md` §5.
//!
//! Two rules:
//! 1. No bare `std::env::var` (or `env::var`) in the scoped paths.
//!    Use `runtime_config::current()` instead.
//! 2. No literal `Duration::from_secs(N)` / `from_millis(N)` /
//!    `from_micros(N)`. Suppress with `// allow: <reason>` if it's a
//!    protocol invariant or test.
//!
//! Exit codes: 0 = clean, 1 = violations found.

use std::process::ExitCode;
use walkdir::WalkDir;

// Stage 2 widening (per RUNTIME_CONFIG_STAGE2_PR_PLAN.md §5).
// Stage 3 will go project-wide.
const STAGE_PATHS: &[&str] = &[
    "highper-gateway/src/plugin",
    "highper-gateway/src/cluster",
    "highper-gateway/src/cache",
    "highper-gateway/src/ai",
];

const ENV_VAR_PATTERNS: &[&str] = &["std::env::var", "env::var("];

const DURATION_PATTERNS: &[&str] = &[
    "Duration::from_secs(",
    "Duration::from_millis(",
    "Duration::from_micros(",
];

fn main() -> ExitCode {
    let mut violations: Vec<String> = Vec::new();

    for root in STAGE_PATHS {
        // Skip non-existent paths silently — `src/ai/` doesn't exist until
        // Phase 2 starts; same for any pre-Stage-3 holes.
        if !std::path::Path::new(root).exists() {
            continue;
        }
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }

            let display_path = path.display().to_string();
            let in_test_module = is_test_file(&display_path);

            let Ok(contents) = std::fs::read_to_string(path) else {
                continue;
            };

            let lines: Vec<&str> = contents.lines().collect();
            let mut in_cfg_test = false;
            for (line_no, raw_line) in lines.iter().enumerate() {
                let line = raw_line.trim_start();

                // Track #[cfg(test)] mod blocks at module level (cheap heuristic).
                if line.starts_with("#[cfg(test)]") {
                    in_cfg_test = true;
                    continue;
                }
                if in_cfg_test && (line.starts_with("mod ") || line.starts_with("pub mod ")) {
                    // Entered the test mod — skip the rest of this file.
                    break;
                }

                if in_test_module {
                    continue;
                }

                // Waivers: `// allow:` may appear on the same line as the violation
                // OR on the immediately-preceding line (idiomatic for long literals).
                let waived = raw_line.contains("// allow:")
                    || (line_no > 0 && lines[line_no - 1].contains("// allow:"));
                if waived {
                    continue;
                }

                for pat in ENV_VAR_PATTERNS {
                    if raw_line.contains(pat) {
                        violations.push(format!(
                            "{}:{}: bare {} — use runtime_config::current()",
                            display_path,
                            line_no + 1,
                            pat
                        ));
                    }
                }

                for pat in DURATION_PATTERNS {
                    if raw_line.contains(pat) {
                        violations.push(format!(
                            "{}:{}: hardcoded {} — load from runtime_config or add `// allow: <reason>`",
                            display_path,
                            line_no + 1,
                            pat
                        ));
                    }
                }
            }
        }
    }

    if violations.is_empty() {
        println!("lint-runtime-config: clean ({} paths checked)", STAGE_PATHS.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("lint-runtime-config: {} violation(s):", violations.len());
        for v in &violations {
            eprintln!("  {v}");
        }
        eprintln!();
        eprintln!("Stage 2 scope: src/plugin/, src/cluster/, src/cache/, src/ai/.");
        eprintln!("Stage 3 will go project-wide.");
        eprintln!("See docs/planning/RUNTIME_CONFIG_STAGE2_PR_PLAN.md §5.");
        ExitCode::FAILURE
    }
}

fn is_test_file(path: &str) -> bool {
    path.contains("/tests/") || path.contains("\\tests\\")
}
