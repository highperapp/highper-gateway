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

use std::fmt;
use std::path::PathBuf;

use crate::runtime_config::error::RuntimeConfigError;

#[derive(Clone)]
pub enum SecretRef {
    Literal(String),
    File { path: PathBuf, lazy: bool },
    /// Resolved via the secrets-resolver selected by `SecretsRuntimeConfig`
    /// (Stage 2 ships the parse path + struct field; actual Vault / AWS /
    /// K8s clients ship in Phase 1.4 — `resolve_eager` returns a clear
    /// "not implemented" error until then).
    Secrets { uri: String, lazy: bool },
}

/// Custom `Debug` redacts secret values so `format!("{:?}", cfg)` and
/// reload-diff output never leak plaintext. The `SecretRef::resolve_eager`
/// call site is the only place the actual value is exposed.
impl fmt::Debug for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecretRef::Literal(_) => f.write_str("SecretRef::Literal(***)"),
            SecretRef::File { path, lazy } => f
                .debug_struct("SecretRef::File")
                .field("path", path)
                .field("lazy", lazy)
                .finish(),
            SecretRef::Secrets { uri, lazy } => f
                .debug_struct("SecretRef::Secrets")
                .field("uri", uri)
                .field("lazy", lazy)
                .finish(),
        }
    }
}

/// Equality for diff comparison. `Literal` compares its plaintext (so a
/// rotation is detected) but the values never reach `Debug`.
impl PartialEq for SecretRef {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SecretRef::Literal(a), SecretRef::Literal(b)) => a == b,
            (
                SecretRef::File { path: a, lazy: la },
                SecretRef::File { path: b, lazy: lb },
            ) => a == b && la == lb,
            (
                SecretRef::Secrets { uri: a, lazy: la },
                SecretRef::Secrets { uri: b, lazy: lb },
            ) => a == b && la == lb,
            _ => false,
        }
    }
}

impl Eq for SecretRef {}

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
        if let Some(rest) = value.strip_prefix("secrets://") {
            let (uri, lazy) = parse_lazy_query(rest);
            return Ok(SecretRef::Secrets {
                uri: uri.to_string(),
                lazy,
            });
        }
        Err(RuntimeConfigError::ParseError {
            env_var: env_var.to_string(),
            value: value.to_string(),
            expected: "literal:<value>, file:///<path>[?lazy=true], or secrets://<uri>[?lazy=true]",
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
            SecretRef::Secrets { uri, lazy: _ } => {
                // Stage 2 stub. Phase 1.4 wires the actual Vault / AWS Secrets
                // Manager / K8s Secret resolvers via SecretsRuntimeConfig.
                Err(RuntimeConfigError::InvalidCombination {
                    rule: "secrets:// resolution not yet implemented",
                    details: format!(
                        "uri={uri}; ships in Phase 1.4 with SecretsRuntimeConfig.provider"
                    ),
                })
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
    fn parses_secrets_uri() {
        let r = SecretRef::parse("HIGHPER_TEST", "secrets://kv/secret/highper").unwrap();
        match r {
            SecretRef::Secrets { uri, lazy } => {
                assert_eq!(uri, "kv/secret/highper");
                assert!(!lazy);
            }
            _ => panic!("expected Secrets"),
        }
    }

    #[test]
    fn parses_secrets_lazy_query() {
        let r = SecretRef::parse("HIGHPER_TEST", "secrets://kv/secret/highper?lazy=true").unwrap();
        match r {
            SecretRef::Secrets { lazy, .. } => assert!(lazy),
            _ => panic!("expected Secrets"),
        }
    }

    #[test]
    fn secrets_resolve_eager_returns_not_implemented() {
        let r = SecretRef::Secrets {
            uri: "kv/foo".into(),
            lazy: false,
        };
        let res = r.resolve_eager();
        assert!(matches!(
            res,
            Err(RuntimeConfigError::InvalidCombination {
                rule: "secrets:// resolution not yet implemented",
                ..
            })
        ));
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
