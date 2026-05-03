//! Tier 1 hot-reload — SIGHUP handler + atomic swap + section-level diff.
//!
//! Per `docs/planning/RUNTIME_CONFIG_STAGE3_PR_PLAN.md` §3.1. Stage 3b lands
//! the runtime; Stage 3c will add the `/admin/config/diff` endpoint that
//! consumes `latest_diff()`.
//!
//! Reload classification:
//! - Fields wrapped in `Reloadable<T>` are Live: swapped immediately by
//!   `arc_swap::ArcSwap::store`, visible to all subsequent `current()` reads.
//! - Bare fields are Restart-required: loaded into the new struct and
//!   reported as pending in the diff, but in-flight workers continue
//!   reading the old value via the previously-loaded `Arc<RuntimeConfig>`
//!   snapshot until the process restarts.
//!
//! Diff granularity: section-level (`cluster`, `ai`, etc.). Field-level
//! granularity is deferred — section-level is sufficient for the operator-
//! facing log line and the diff endpoint, and cheap to compute via the
//! standard `Debug`-string comparison (which is already secret-safe thanks
//! to the custom `SecretRef::Debug` impl in `secret_ref.rs`).

use std::sync::Arc;

/// Marks a field as live-reloadable on SIGHUP (atomic swap is safe).
/// Bare fields (without this wrapper) are Restart-required.
///
/// Field-level Live/Restart classification — Stage 3b currently only uses
/// section-level diffs, but the marker is preserved so a future
/// implementation can do field-level reporting without changing the type
/// surface of every section.
#[derive(Debug, Clone)]
pub struct Reloadable<T> {
    inner: T,
}

impl<T> Reloadable<T> {
    pub fn new(value: T) -> Self {
        Self { inner: value }
    }

    pub fn get(&self) -> &T {
        &self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: Default> Default for Reloadable<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// Section-level diff between two `RuntimeConfig` snapshots.
#[derive(Debug, Clone)]
pub struct ReloadDiff {
    /// Section names whose `Debug` output changed. Includes both Live and
    /// Restart fields — operators consult the per-section
    /// reload-tier metadata (in `docs/CONFIG_ENV.md`) to interpret each.
    pub changed_sections: Vec<&'static str>,
    /// Wall-clock time the diff was computed.
    pub timestamp: std::time::SystemTime,
}

impl Default for ReloadDiff {
    fn default() -> Self {
        Self::empty()
    }
}

impl ReloadDiff {
    pub fn empty() -> Self {
        Self {
            changed_sections: Vec::new(),
            timestamp: std::time::SystemTime::now(),
        }
    }
}

/// Compare two `RuntimeConfig` snapshots and return a section-level diff.
/// Uses `Debug`-string comparison per section — cheap, deterministic, and
/// secret-safe via the custom `SecretRef::Debug` impl.
pub fn compute_diff(
    old: &super::RuntimeConfig,
    new: &super::RuntimeConfig,
) -> ReloadDiff {
    let mut changed = Vec::new();

    // Each entry: (section name, did-change). Section-level granularity is
    // sufficient for the operator-facing log line; field-level is a future
    // enhancement when the admin endpoint needs to report individual
    // field changes.
    macro_rules! check {
        ($name:literal, $field:ident) => {
            if format!("{:?}", old.$field) != format!("{:?}", new.$field) {
                changed.push($name);
            }
        };
    }

    check!("cluster", cluster);
    check!("plugin", plugin);
    check!("ai", ai);
    check!("body", body);
    check!("shutdown", shutdown);
    check!("secrets", secrets);
    check!("http3", http3);
    check!("tls", tls);
    check!("ratelimit", ratelimit);
    check!("circuit_breaker", circuit_breaker);
    check!("geo", geo);
    check!("cache", cache);
    check!("signals", signals);
    check!("config_watcher", config_watcher);
    check!("observability", observability);

    ReloadDiff {
        changed_sections: changed,
        timestamp: std::time::SystemTime::now(),
    }
}

/// Re-run `load()`, atomically swap the global `RuntimeConfig`, and
/// return the diff. Live-classified fields take effect immediately on
/// the next `current()` read; Restart-classified fields are loaded into
/// the new struct but in-flight workers continue using the old snapshot
/// they already loaded.
///
/// Returns the diff so the caller can log it / persist it.
pub fn reload_now() -> Result<ReloadDiff, super::RuntimeConfigError> {
    let new_cfg = super::load()?;
    let old_cfg = super::current();
    let diff = compute_diff(&old_cfg, &new_cfg);

    let arc_swap = super::current_arcswap();
    arc_swap.store(Arc::new(new_cfg));

    super::store_latest_diff(diff.clone());
    Ok(diff)
}

/// Spawn a Tokio task that listens for SIGHUP and runs `reload_now()` on
/// each signal. The handler logs the resulting diff via `tracing`.
///
/// **Chained with existing config-file-reload** per
/// `RUNTIME_CONFIG_STAGE3_PR_PLAN.md` §11 #4: this handler co-exists with
/// the `signal-hook-tokio` listener that the `Reload` CLI command sends
/// SIGHUP to (per `src/main.rs` reload_command). Both fire on every SIGHUP.
#[cfg(unix)]
pub async fn install_sighup_handler() -> anyhow::Result<()> {
    use signal_hook::consts::SIGHUP;
    use signal_hook_tokio::Signals;
    use futures::stream::StreamExt;

    let mut signals = Signals::new([SIGHUP])?;
    tokio::spawn(async move {
        while let Some(_signal) = signals.next().await {
            tracing::info!("SIGHUP received — reloading RuntimeConfig");
            match reload_now() {
                Ok(diff) => {
                    if diff.changed_sections.is_empty() {
                        tracing::info!("RuntimeConfig reload: no changes detected");
                    } else {
                        tracing::info!(
                            "RuntimeConfig reload: {} section(s) changed: {:?}",
                            diff.changed_sections.len(),
                            diff.changed_sections,
                        );
                    }
                }
                Err(e) => tracing::error!("RuntimeConfig reload failed: {e}"),
            }
        }
    });
    Ok(())
}

/// Windows fallback — SIGHUP is Unix-only; Windows operators restart the
/// process for config changes (the existing `Reload` CLI command also
/// no-ops on Windows per `main.rs::reload_command`).
#[cfg(not(unix))]
pub async fn install_sighup_handler() -> anyhow::Result<()> {
    tracing::info!(
        "SIGHUP runtime-config reload not supported on this platform; \
         restart the process to apply HIGHPER_* env-var changes"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_config::RuntimeConfig;

    #[test]
    fn empty_diff_for_identical_configs() {
        let cfg = RuntimeConfig::for_test();
        let diff = compute_diff(&cfg, &cfg);
        assert!(diff.changed_sections.is_empty());
    }

    #[test]
    fn diff_detects_section_change() {
        let mut a = RuntimeConfig::for_test();
        let mut b = RuntimeConfig::for_test();
        // Mutate the plugin section.
        b.plugin.drain = Reloadable::new(std::time::Duration::from_secs(60));
        let diff = compute_diff(&a, &b);
        assert_eq!(diff.changed_sections, vec!["plugin"]);

        // Identical again after copying.
        a.plugin.drain = Reloadable::new(std::time::Duration::from_secs(60));
        let diff = compute_diff(&a, &b);
        assert!(diff.changed_sections.is_empty());
    }
}
