//! GraphQL request-shape limits (B5 — Workstream 0.E).
//!
//! Operator-tunable thresholds applied to every GraphQL request after
//! `parse_query` succeeds. Rejecting unbounded queries early prevents a
//! single client from forcing the federation executor to walk an
//! arbitrarily-deep AST or fan out to thousands of fields.
//!
//! Defaults (15 / 1000) match the Apollo / async-graphql convention.

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct GraphqlRuntimeConfig {
    /// Reject queries whose nesting depth exceeds this value.
    pub max_depth: Reloadable<u32>, // HIGHPER_GRAPHQL_MAX_DEPTH (default 15)
    /// Reject queries whose visited-field count exceeds this value.
    pub max_complexity: Reloadable<u32>, // HIGHPER_GRAPHQL_MAX_COMPLEXITY (default 1000)
    /// When false, the analyzer is bypassed (recordable warning instead
    /// of a hard reject). Useful for staged rollouts where operators
    /// want to log+observe before enforcing.
    pub enforce: Reloadable<bool>, // HIGHPER_GRAPHQL_ENFORCE_LIMITS (default true)
}

impl Default for GraphqlRuntimeConfig {
    fn default() -> Self {
        Self {
            max_depth: Reloadable::new(15),
            max_complexity: Reloadable::new(1000),
            enforce: Reloadable::new(true),
        }
    }
}

pub(crate) fn load() -> Result<GraphqlRuntimeConfig, RuntimeConfigError> {
    let max_depth = parse_u32(
        "HIGHPER_GRAPHQL_MAX_DEPTH",
        env_string("GRAPHQL_MAX_DEPTH").as_deref(),
        15,
    )?;
    if max_depth == 0 {
        return Err(RuntimeConfigError::OutOfRange {
            env_var: "HIGHPER_GRAPHQL_MAX_DEPTH".into(),
            value: "0".into(),
            valid_range: ">= 1",
        });
    }

    let max_complexity = parse_u32(
        "HIGHPER_GRAPHQL_MAX_COMPLEXITY",
        env_string("GRAPHQL_MAX_COMPLEXITY").as_deref(),
        1000,
    )?;
    if max_complexity == 0 {
        return Err(RuntimeConfigError::OutOfRange {
            env_var: "HIGHPER_GRAPHQL_MAX_COMPLEXITY".into(),
            value: "0".into(),
            valid_range: ">= 1",
        });
    }

    let enforce = match env_string("GRAPHQL_ENFORCE_LIMITS").as_deref() {
        None | Some("true") | Some("1") => true,
        Some("false") | Some("0") => false,
        Some(o) => {
            return Err(RuntimeConfigError::ParseError {
                env_var: "HIGHPER_GRAPHQL_ENFORCE_LIMITS".into(),
                value: o.into(),
                expected: "true|false (or 1|0)",
            });
        }
    };

    Ok(GraphqlRuntimeConfig {
        max_depth: Reloadable::new(max_depth),
        max_complexity: Reloadable::new(max_complexity),
        enforce: Reloadable::new(enforce),
    })
}

fn parse_u32(env_var: &str, raw: Option<&str>, default: u32) -> Result<u32, RuntimeConfigError> {
    match raw {
        None => Ok(default),
        Some(s) => s
            .trim()
            .parse::<u32>()
            .map_err(|_| RuntimeConfigError::ParseError {
                env_var: env_var.into(),
                value: s.into(),
                expected: "u32",
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear() {
        for k in [
            "HIGHPER_GRAPHQL_MAX_DEPTH",
            "HIGHPER_GRAPHQL_MAX_COMPLEXITY",
            "HIGHPER_GRAPHQL_ENFORCE_LIMITS",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn defaults_match_apollo_convention() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(*cfg.max_depth.get(), 15);
        assert_eq!(*cfg.max_complexity.get(), 1000);
        assert!(*cfg.enforce.get());
    }

    #[test]
    #[serial]
    fn rejects_zero_depth() {
        clear();
        std::env::set_var("HIGHPER_GRAPHQL_MAX_DEPTH", "0");
        assert!(matches!(load(), Err(RuntimeConfigError::OutOfRange { .. })));
        clear();
    }

    #[test]
    #[serial]
    fn rejects_zero_complexity() {
        clear();
        std::env::set_var("HIGHPER_GRAPHQL_MAX_COMPLEXITY", "0");
        assert!(matches!(load(), Err(RuntimeConfigError::OutOfRange { .. })));
        clear();
    }

    #[test]
    #[serial]
    fn parses_enforce_false() {
        clear();
        std::env::set_var("HIGHPER_GRAPHQL_ENFORCE_LIMITS", "false");
        let cfg = load().unwrap();
        assert!(!*cfg.enforce.get());
        clear();
    }

    #[test]
    #[serial]
    fn rejects_bad_enforce_value() {
        clear();
        std::env::set_var("HIGHPER_GRAPHQL_ENFORCE_LIMITS", "maybe");
        assert!(matches!(load(), Err(RuntimeConfigError::ParseError { .. })));
        clear();
    }
}
