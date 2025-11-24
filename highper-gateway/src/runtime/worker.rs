//! Worker pool implementation

use tracing::info;

/// A worker thread for handling connections
pub struct Worker {
    pub id: usize,
}

impl Worker {
    /// Create a new worker with the given ID
    pub fn new(id: usize) -> Self {
        info!("Worker {} initialized", id);
        Self { id }
    }

    /// Run the worker
    pub async fn run(&self) {
        info!("Worker {} started", self.id);
        // Worker implementation will be added in later phases
    }
}
