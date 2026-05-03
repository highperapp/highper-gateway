//! Observability backend selectors (Phase 1.4 prep; §4.4 row 7).

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone, Default)]
pub struct ObservabilityRuntimeConfig {
    pub metrics_backend: Reloadable<MetricsBackend>,    // HIGHPER_OBS_METRICS_BACKEND
    pub log_backend: Reloadable<LogBackend>,            // HIGHPER_OBS_LOG_BACKEND
    pub log_format: Reloadable<LogFormat>,              // HIGHPER_OBS_LOG_FORMAT
    pub log_level: Reloadable<LogLevel>,                // HIGHPER_OBS_LOG_LEVEL
    pub trace_sampling_rate: Reloadable<f64>,           // HIGHPER_OBS_TRACE_SAMPLING (0.0..=1.0)
    pub otlp_endpoint: Reloadable<Option<String>>,      // HIGHPER_OBS_OTLP_ENDPOINT
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MetricsBackend {
    #[default]
    Prometheus,
    Otlp,
    None,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogBackend {
    #[default]
    Stderr,
    Stdout,
    File,
    Otlp,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogFormat {
    #[default]
    Pretty,
    Json,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

pub(crate) fn load() -> Result<ObservabilityRuntimeConfig, RuntimeConfigError> {
    let metrics_backend = match env_string("OBS_METRICS_BACKEND").as_deref() {
        None | Some("prometheus") => MetricsBackend::Prometheus,
        Some("otlp") => MetricsBackend::Otlp,
        Some("none") => MetricsBackend::None,
        Some(o) => return Err(parse_err("HIGHPER_OBS_METRICS_BACKEND", o, "prometheus|otlp|none")),
    };
    let log_backend = match env_string("OBS_LOG_BACKEND").as_deref() {
        None | Some("stderr") => LogBackend::Stderr,
        Some("stdout") => LogBackend::Stdout,
        Some("file") => LogBackend::File,
        Some("otlp") => LogBackend::Otlp,
        Some(o) => return Err(parse_err("HIGHPER_OBS_LOG_BACKEND", o, "stderr|stdout|file|otlp")),
    };
    let log_format = match env_string("OBS_LOG_FORMAT").as_deref() {
        None | Some("pretty") => LogFormat::Pretty,
        Some("json") => LogFormat::Json,
        Some(o) => return Err(parse_err("HIGHPER_OBS_LOG_FORMAT", o, "pretty|json")),
    };
    let log_level = match env_string("OBS_LOG_LEVEL").as_deref() {
        None | Some("info") => LogLevel::Info,
        Some("trace") => LogLevel::Trace,
        Some("debug") => LogLevel::Debug,
        Some("warn") => LogLevel::Warn,
        Some("error") => LogLevel::Error,
        Some(o) => return Err(parse_err("HIGHPER_OBS_LOG_LEVEL", o, "trace|debug|info|warn|error")),
    };
    let trace_sampling_rate = match env_string("OBS_TRACE_SAMPLING") {
        None => 0.1,
        Some(s) => {
            let v = s.trim().parse::<f64>().map_err(|_| RuntimeConfigError::ParseError {
                env_var: "HIGHPER_OBS_TRACE_SAMPLING".into(),
                value: s.clone(),
                expected: "f64 in [0.0, 1.0]",
            })?;
            if !(0.0..=1.0).contains(&v) {
                return Err(RuntimeConfigError::OutOfRange {
                    env_var: "HIGHPER_OBS_TRACE_SAMPLING".into(),
                    value: s,
                    valid_range: "0.0..=1.0",
                });
            }
            v
        }
    };
    let otlp_endpoint = env_string("OBS_OTLP_ENDPOINT");

    Ok(ObservabilityRuntimeConfig {
        metrics_backend: Reloadable::new(metrics_backend),
        log_backend: Reloadable::new(log_backend),
        log_format: Reloadable::new(log_format),
        log_level: Reloadable::new(log_level),
        trace_sampling_rate: Reloadable::new(trace_sampling_rate),
        otlp_endpoint: Reloadable::new(otlp_endpoint),
    })
}

fn parse_err(env_var: &str, value: &str, expected: &'static str) -> RuntimeConfigError {
    RuntimeConfigError::ParseError {
        env_var: env_var.into(),
        value: value.into(),
        expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear() {
        for k in [
            "HIGHPER_OBS_METRICS_BACKEND",
            "HIGHPER_OBS_LOG_BACKEND",
            "HIGHPER_OBS_LOG_FORMAT",
            "HIGHPER_OBS_LOG_LEVEL",
            "HIGHPER_OBS_TRACE_SAMPLING",
            "HIGHPER_OBS_OTLP_ENDPOINT",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn defaults_prometheus_stderr_pretty_info() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(*cfg.metrics_backend.get(), MetricsBackend::Prometheus);
        assert_eq!(*cfg.log_backend.get(), LogBackend::Stderr);
        assert_eq!(*cfg.log_format.get(), LogFormat::Pretty);
        assert_eq!(*cfg.log_level.get(), LogLevel::Info);
        assert!((cfg.trace_sampling_rate.get() - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    #[serial]
    fn rejects_trace_sampling_above_1() {
        clear();
        std::env::set_var("HIGHPER_OBS_TRACE_SAMPLING", "1.5");
        assert!(matches!(load(), Err(RuntimeConfigError::OutOfRange { .. })));
        clear();
    }
}
