//! Operator-friendly errors for runtime configuration loading.
//!
//! Every variant carries enough context (env-var name, observed value, expected
//! shape) to let the operator correct the misconfiguration without reading
//! source. Per `docs/planning/SETTINGS_SCAFFOLD.md` §4 validation policy.

use std::fmt;

#[derive(Debug)]
pub enum RuntimeConfigError {
    /// Env var was set but couldn't be parsed.
    ParseError {
        env_var: String,
        value: String,
        expected: &'static str,
    },
    /// Env var value is outside the documented valid range.
    OutOfRange {
        env_var: String,
        value: String,
        valid_range: &'static str,
    },
    /// Cross-subsystem invariant violated.
    InvalidCombination { rule: &'static str, details: String },
    /// Required env var missing for the enabled feature set.
    MissingRequired {
        env_var: String,
        required_because: &'static str,
    },
}

impl fmt::Display for RuntimeConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError {
                env_var,
                value,
                expected,
            } => write!(
                f,
                "{env_var}={value:?} could not be parsed (expected {expected})"
            ),
            Self::OutOfRange {
                env_var,
                value,
                valid_range,
            } => write!(
                f,
                "{env_var}={value:?} is out of range (valid: {valid_range})"
            ),
            Self::InvalidCombination { rule, details } => {
                write!(f, "invalid configuration combination: {rule} ({details})")
            }
            Self::MissingRequired {
                env_var,
                required_because,
            } => write!(f, "{env_var} is required because {required_because}"),
        }
    }
}

impl std::error::Error for RuntimeConfigError {}
