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

// Hard-fail scope: paths where migration to RuntimeConfig is complete.
// Adding a literal here regresses the env-var-only rule and fails CI.
//
// Stage 1: src/plugin/. Stage 2 widened to + src/cluster/ + src/cache/ +
// src/ai/. Stage 3 confirms src/cache/ now reads CacheRuntimeConfig
// (10 of 11 Stage 2 waivers resolved; the 11th is a doc-comment example
// kept under `// allow: doc-comment example`).
//
// Project-wide enforcement: a project-wide survey at Stage 3 surfaced
// ~150 pre-existing literals across `src/admin/`, `src/discovery/`,
// `src/gateway/`, `src/proxy/`, `src/middleware/`, etc. — too many to
// migrate in a single PR. They are tracked for Stage 4+ workstreams as
// per-subsystem migrations; not blocking CI today.
const STAGE_PATHS: &[&str] = &[
    "highper-gateway/src/plugin",
    "highper-gateway/src/cluster",
    "highper-gateway/src/cache",
    "highper-gateway/src/ai",
];

// File / directory skips (everything matching skipped). Use for files where
// every literal is intentional and any migration is a separate, scoped PR.
const SKIP_FILES: &[&str] = &[
    // The entire `src/config/` directory is the user-facing config-file
    // parser layer:
    // - `env_override.rs` IS the `std::env::var` parser layer that
    //   `runtime_config/` builds on; bare env::var calls here are the
    //   legitimate primitive (the lint exists to forbid bare env::var
    //   *outside* this layer).
    // - `defaults.rs` is per-protocol presets (MySQL/PostgreSQL/Redis/HTTP/
    //   gRPC/WebSocket/GraphQL/static/PHP-FPM). Migrating these to
    //   RuntimeConfig requires a per-protocol design pass; out of scope
    //   for Workstream 0.J Stage 3.
    // - `schema.rs` / `dsl_ast.rs` carry default values for the
    //   user-facing `Config` struct (file-driven), not RuntimeConfig
    //   (env-driven). Distinct concept.
    // - `reloader.rs` / `validator.rs` / `watcher.rs` / `validation.rs`
    //   are the config-file lifecycle.
    "src/config/",
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
            // Normalize Windows backslashes for SKIP_FILES match.
            let normalized = display_path.replace('\\', "/");
            if SKIP_FILES.iter().any(|skip| normalized.contains(skip)) {
                continue;
            }
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
                    // Only flag when the argument is a numeric literal —
                    // `Duration::from_secs(60)` is a violation;
                    // `Duration::from_secs(secs)` or
                    // `Duration::from_secs(*expr.get())` is a legitimate
                    // call site reading from a variable / RuntimeConfig.
                    if let Some(idx) = raw_line.find(pat) {
                        let after = &raw_line[idx + pat.len()..];
                        let next_ch = after.chars().next().unwrap_or(' ');
                        if next_ch.is_ascii_digit() {
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
        eprintln!("Hard-fail scope: src/plugin/, src/cluster/, src/cache/, src/ai/.");
        eprintln!("(Project-wide survey of ~150 pre-existing literals tracked for Stage 4+.)");
        eprintln!("See docs/planning/RUNTIME_CONFIG_STAGE3_PR_PLAN.md §5.");
        ExitCode::FAILURE
    }
}

fn is_test_file(path: &str) -> bool {
    path.contains("/tests/") || path.contains("\\tests\\")
}
