//! Pointer to an externally-stored secret with eager-by-default resolution.
//!
//! Stage 1 ships `Literal` and `File` variants only. `Secrets://` URI variant
//! lands in Stage 2 with `SecretsRuntimeConfig` per
//! `docs/planning/SETTINGS_SCAFFOLD.md` §6.
//!
//! URI grammar:
//! - `literal:<value>` — direct value (use sparingly; prefer file:// or secrets://)
//! - `file:///<path>` — read from filesystem at boot (eager) or first use (lazy)
//! - `file:///<path>?lazy=true` — defer file read to first use

use std::path::PathBuf;

use crate::runtime_config::error::RuntimeConfigError;

#[derive(Debug, Clone)]
pub enum SecretRef {
    Literal(String),
    File { path: PathBuf, lazy: bool },
    // Secrets { uri: String, lazy: bool },  // Stage 2 — needs SecretsRuntimeConfig
}

#[derive(Debug, Clone)]
pub struct SecretValue(pub String);

impl SecretRef {
    /// Parse from a raw env-var value.
    pub fn parse(env_var: &str, value: &str) -> Result<Self, RuntimeConfigError> {
        if let Some(literal) = value.strip_prefix("literal:") {
            return Ok(SecretRef::Literal(literal.to_string()));
        }
        if let Some(rest) = value.strip_prefix("file://") {
            let (path_str, lazy) = parse_lazy_query(rest);
            // Tolerate the leading "/" that follows file:// in some URIs.
            let stripped = path_str.strip_prefix('/').unwrap_or(path_str);
            return Ok(SecretRef::File {
                path: PathBuf::from(stripped),
                lazy,
            });
        }
        Err(RuntimeConfigError::ParseError {
            env_var: env_var.to_string(),
            value: value.to_string(),
            expected: "literal:<value> or file:///<path>[?lazy=true]",
        })
    }

    /// Eagerly resolve at boot; returns `None` for `lazy=true` File variants.
    pub fn resolve_eager(&self) -> Result<Option<SecretValue>, RuntimeConfigError> {
        match self {
            SecretRef::Literal(s) => Ok(Some(SecretValue(s.clone()))),
            SecretRef::File { path, lazy } => {
                if *lazy {
                    return Ok(None);
                }
                let contents = std::fs::read_to_string(path).map_err(|e| {
                    RuntimeConfigError::InvalidCombination {
                        rule: "secret file unreadable",
                        details: format!("{}: {}", path.display(), e),
                    }
                })?;
                Ok(Some(SecretValue(contents.trim_end().to_string())))
            }
        }
    }
}

fn parse_lazy_query(rest: &str) -> (&str, bool) {
    if let Some((path, query)) = rest.split_once('?') {
        let lazy = query
            .split('&')
            .any(|kv| kv == "lazy=true" || kv == "lazy=1");
        (path, lazy)
    } else {
        (rest, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_literal() {
        let r = SecretRef::parse("HIGHPER_TEST", "literal:hunter2").unwrap();
        match r {
            SecretRef::Literal(s) => assert_eq!(s, "hunter2"),
            _ => panic!("expected Literal"),
        }
    }

    #[test]
    fn parses_file_eager() {
        let r = SecretRef::parse("HIGHPER_TEST", "file:///etc/highper/secret").unwrap();
        match r {
            SecretRef::File { path, lazy } => {
                assert_eq!(path, PathBuf::from("etc/highper/secret"));
                assert!(!lazy);
            }
            _ => panic!("expected File"),
        }
    }

    #[test]
    fn parses_file_lazy_query() {
        let r = SecretRef::parse("HIGHPER_TEST", "file:///etc/highper/secret?lazy=true").unwrap();
        match r {
            SecretRef::File { lazy, .. } => assert!(lazy),
            _ => panic!("expected File"),
        }
    }

    #[test]
    fn rejects_unknown_scheme() {
        let r = SecretRef::parse("HIGHPER_TEST", "vault://kv/secret");
        assert!(matches!(r, Err(RuntimeConfigError::ParseError { .. })));
    }

    #[test]
    fn literal_resolves_eagerly() {
        let r = SecretRef::Literal("foo".into());
        let v = r.resolve_eager().unwrap().unwrap();
        assert_eq!(v.0, "foo");
    }

    #[test]
    fn lazy_file_returns_none_eagerly() {
        let r = SecretRef::File {
            path: PathBuf::from("nonexistent"),
            lazy: true,
        };
        let v = r.resolve_eager().unwrap();
        assert!(v.is_none());
    }
}
