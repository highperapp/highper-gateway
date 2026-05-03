//! Field-level reloadability marker.
//!
//! Fields wrapped in `Reloadable<T>` are Live: the Tier 1 SIGHUP handler
//! (Phase 0.J Stage 3) atomically swaps them in place. Bare fields are
//! Restart-required: loaded into the new struct on SIGHUP but reported as
//! pending until the next process start.
//!
//! Stage 1 ships only the marker type. The reload runtime, diff report,
//! and `/admin/config/diff` endpoint land in Stage 3 per
//! `docs/planning/SETTINGS_SCAFFOLD.md` §5.

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
