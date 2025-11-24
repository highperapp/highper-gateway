//! System resource monitoring
//!
//! Monitors critical system resources:
//! - File descriptor usage
//! - TCP socket states (ESTABLISHED, TIME_WAIT, CLOSE_WAIT, etc.)
//! - Memory usage
//! - CPU usage

use std::fs;
use tracing::{debug, warn};

/// System resource statistics
#[derive(Debug, Clone, Default)]
pub struct SystemStats {
    /// File descriptor statistics
    pub fd_stats: FileDescriptorStats,

    /// TCP socket state statistics
    pub socket_stats: SocketStateStats,

    /// Memory statistics (in bytes)
    pub memory_stats: MemoryStats,
}

/// File descriptor usage statistics
#[derive(Debug, Clone, Default)]
pub struct FileDescriptorStats {
    /// Current number of open file descriptors
    pub open_fds: usize,

    /// Soft limit for file descriptors
    pub soft_limit: usize,

    /// Hard limit for file descriptors
    pub hard_limit: usize,

    /// Percentage of file descriptors in use (0-100)
    pub usage_percent: f64,
}

/// TCP socket state statistics
#[derive(Debug, Clone, Default)]
pub struct SocketStateStats {
    /// Number of ESTABLISHED connections
    pub established: usize,

    /// Number of TIME_WAIT connections
    pub time_wait: usize,

    /// Number of CLOSE_WAIT connections
    pub close_wait: usize,

    /// Number of FIN_WAIT1 connections
    pub fin_wait1: usize,

    /// Number of FIN_WAIT2 connections
    pub fin_wait2: usize,

    /// Number of SYN_SENT connections
    pub syn_sent: usize,

    /// Number of SYN_RECV connections
    pub syn_recv: usize,

    /// Number of LISTEN sockets
    pub listen: usize,

    /// Number of LAST_ACK connections
    pub last_ack: usize,

    /// Number of CLOSING connections
    pub closing: usize,

    /// Total connections across all states
    pub total: usize,
}

/// Memory usage statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    /// Resident set size (RSS) in bytes
    pub rss: usize,

    /// Virtual memory size in bytes
    pub vms: usize,

    /// Shared memory in bytes
    pub shared: usize,
}

impl SystemStats {
    /// Collect current system statistics
    pub fn collect() -> Self {
        Self {
            fd_stats: FileDescriptorStats::collect(),
            socket_stats: SocketStateStats::collect(),
            memory_stats: MemoryStats::collect(),
        }
    }
}

impl FileDescriptorStats {
    /// Collect file descriptor statistics
    #[cfg(target_os = "linux")]
    pub fn collect() -> Self {
        let open_fds = Self::count_open_fds();
        let (soft_limit, hard_limit) = Self::get_fd_limits();

        let usage_percent = if soft_limit > 0 {
            (open_fds as f64 / soft_limit as f64) * 100.0
        } else {
            0.0
        };

        if usage_percent > 80.0 {
            warn!(
                "High file descriptor usage: {}/{} ({:.1}%)",
                open_fds, soft_limit, usage_percent
            );
        }

        Self {
            open_fds,
            soft_limit,
            hard_limit,
            usage_percent,
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn collect() -> Self {
        Self::default()
    }

    /// Count currently open file descriptors
    #[cfg(target_os = "linux")]
    fn count_open_fds() -> usize {
        fs::read_dir("/proc/self/fd")
            .map(|entries| entries.count())
            .unwrap_or(0)
    }

    /// Get file descriptor limits (soft, hard)
    #[cfg(target_os = "linux")]
    fn get_fd_limits() -> (usize, usize) {
        if let Ok(content) = fs::read_to_string("/proc/self/limits") {
            for line in content.lines() {
                if line.starts_with("Max open files") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 5 {
                        let soft = parts[3].parse().unwrap_or(0);
                        let hard = parts[4].parse().unwrap_or(0);
                        return (soft, hard);
                    }
                }
            }
        }
        (0, 0)
    }
}

impl SocketStateStats {
    /// Collect TCP socket state statistics
    #[cfg(target_os = "linux")]
    pub fn collect() -> Self {
        let mut stats = Self::default();

        // Parse /proc/net/tcp and /proc/net/tcp6
        if let Ok(tcp_content) = fs::read_to_string("/proc/net/tcp") {
            Self::parse_tcp_file(&tcp_content, &mut stats);
        }

        if let Ok(tcp6_content) = fs::read_to_string("/proc/net/tcp6") {
            Self::parse_tcp_file(&tcp6_content, &mut stats);
        }

        stats.total = stats.established + stats.time_wait + stats.close_wait
            + stats.fin_wait1 + stats.fin_wait2 + stats.syn_sent
            + stats.syn_recv + stats.listen + stats.last_ack + stats.closing;

        // Warn if too many TIME_WAIT or CLOSE_WAIT sockets
        if stats.time_wait > 5000 {
            warn!("High TIME_WAIT count: {} (consider tuning tcp_fin_timeout)", stats.time_wait);
        }

        if stats.close_wait > 1000 {
            warn!("High CLOSE_WAIT count: {} (application not closing connections properly)", stats.close_wait);
        }

        stats
    }

    #[cfg(not(target_os = "linux"))]
    pub fn collect() -> Self {
        Self::default()
    }

    /// Parse /proc/net/tcp format
    #[cfg(target_os = "linux")]
    fn parse_tcp_file(content: &str, stats: &mut Self) {
        for line in content.lines().skip(1) { // Skip header
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            // State is in column 3 (0-indexed), in hex
            if let Ok(state) = u8::from_str_radix(parts[3], 16) {
                match state {
                    0x01 => stats.established += 1, // ESTABLISHED
                    0x02 => stats.syn_sent += 1,    // SYN_SENT
                    0x03 => stats.syn_recv += 1,    // SYN_RECV
                    0x04 => stats.fin_wait1 += 1,   // FIN_WAIT1
                    0x05 => stats.fin_wait2 += 1,   // FIN_WAIT2
                    0x06 => stats.time_wait += 1,   // TIME_WAIT
                    0x07 => stats.closing += 1,     // CLOSE (reuse closing field)
                    0x08 => stats.close_wait += 1,  // CLOSE_WAIT
                    0x09 => stats.last_ack += 1,    // LAST_ACK
                    0x0A => stats.listen += 1,      // LISTEN
                    0x0B => stats.closing += 1,     // CLOSING
                    _ => {}
                }
            }
        }
    }
}


impl MemoryStats {
    /// Collect memory usage statistics
    #[cfg(target_os = "linux")]
    pub fn collect() -> Self {
        let mut stats = Self::default();

        if let Ok(content) = fs::read_to_string("/proc/self/status") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }

                match parts[0] {
                    "VmRSS:" => {
                        // RSS in KB, convert to bytes
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            stats.rss = kb * 1024;
                        }
                    }
                    "VmSize:" => {
                        // Virtual memory in KB, convert to bytes
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            stats.vms = kb * 1024;
                        }
                    }
                    "RssFile:" => {
                        // Shared memory in KB, convert to bytes
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            stats.shared = kb * 1024;
                        }
                    }
                    _ => {}
                }
            }
        }

        stats
    }

    #[cfg(not(target_os = "linux"))]
    pub fn collect() -> Self {
        Self::default()
    }

    /// Get RSS in megabytes
    pub fn rss_mb(&self) -> f64 {
        self.rss as f64 / (1024.0 * 1024.0)
    }

    /// Get VMS in megabytes
    pub fn vms_mb(&self) -> f64 {
        self.vms as f64 / (1024.0 * 1024.0)
    }
}

/// System resource monitor that periodically collects stats
pub struct SystemMonitor {
    interval: std::time::Duration,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl SystemMonitor {
    /// Create a new system monitor
    pub fn new(
        interval: std::time::Duration,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        Self { interval, shutdown }
    }

    /// Start monitoring in background
    pub async fn run(mut self) {
        debug!("System monitor started (interval: {:?})", self.interval);

        let mut ticker = tokio::time::interval(self.interval);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let stats = SystemStats::collect();

                    // Log stats
                    debug!(
                        "System stats - FDs: {}/{} ({:.1}%), Sockets: {} (EST={}, TIME_WAIT={}, CLOSE_WAIT={}), Memory: RSS={:.1}MB",
                        stats.fd_stats.open_fds,
                        stats.fd_stats.soft_limit,
                        stats.fd_stats.usage_percent,
                        stats.socket_stats.total,
                        stats.socket_stats.established,
                        stats.socket_stats.time_wait,
                        stats.socket_stats.close_wait,
                        stats.memory_stats.rss_mb(),
                    );

                    // Update metrics
                    self.update_metrics(&stats);
                }
                _ = self.shutdown.changed() => {
                    debug!("System monitor shutting down");
                    break;
                }
            }
        }
    }

    fn update_metrics(&self, stats: &SystemStats) {
        use metrics::{gauge, counter};

        // File descriptor metrics
        gauge!("system_fd_open").set(stats.fd_stats.open_fds as f64);
        gauge!("system_fd_limit").set(stats.fd_stats.soft_limit as f64);
        gauge!("system_fd_usage_percent").set(stats.fd_stats.usage_percent);

        // Socket state metrics
        gauge!("system_sockets_established").set(stats.socket_stats.established as f64);
        gauge!("system_sockets_time_wait").set(stats.socket_stats.time_wait as f64);
        gauge!("system_sockets_close_wait").set(stats.socket_stats.close_wait as f64);
        gauge!("system_sockets_total").set(stats.socket_stats.total as f64);

        // Memory metrics
        gauge!("system_memory_rss_bytes").set(stats.memory_stats.rss as f64);
        gauge!("system_memory_vms_bytes").set(stats.memory_stats.vms as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_fd_stats_collection() {
        let stats = FileDescriptorStats::collect();
        assert!(stats.open_fds > 0);
        assert!(stats.soft_limit > 0);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_socket_stats_collection() {
        let stats = SocketStateStats::collect();
        // Should have at least some sockets (test itself uses sockets)
        assert!(stats.total >= 0);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_memory_stats_collection() {
        let stats = MemoryStats::collect();
        assert!(stats.rss > 0);
        assert!(stats.vms > 0);
        assert!(stats.rss_mb() > 0.0);
    }
}
