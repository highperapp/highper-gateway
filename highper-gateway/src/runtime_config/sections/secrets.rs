//! Secrets-resolver provider selector (Phase 1.4 prep).
//!
//! Stage 2 lands the *struct + selector* only. The actual resolver
//! implementations (Vault HTTP client, AWS Secrets Manager SDK, K8s API
//! client) ship in Phase 1.4 and consume `RuntimeConfig::secrets`.

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError, SecretRef};

#[derive(Debug, Clone, Default)]
pub struct SecretsRuntimeConfig {
    /// `HIGHPER_SECRETS_PROVIDER` — which resolver to use for `secrets://` URIs.
    pub provider: SecretsProvider,
    /// `HIGHPER_SECRETS_VAULT_URL` — Vault server URL (when provider=vault).
    pub vault_url: Option<String>,
    /// `HIGHPER_SECRETS_VAULT_TOKEN` — Vault token (literal/file/env-resolved).
    pub vault_token: Option<SecretRef>,
    /// `HIGHPER_SECRETS_AWS_REGION` — AWS region (when provider=aws).
    pub aws_region: Option<String>,
    /// `HIGHPER_SECRETS_K8S_NAMESPACE` — Kubernetes namespace (when provider=k8s).
    pub k8s_namespace: Option<String>,
    /// `HIGHPER_SECRETS_CACHE_TTL` — resolved-secret cache TTL in seconds (default 300).
    pub cache_ttl_secs: Reloadable<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SecretsProvider {
    /// Resolve directly from process env vars (fallback default).
    #[default]
    Env,
    /// Resolve from filesystem (e.g., K8s mounted secrets, Vault sidecar).
    File,
    /// HashiCorp Vault HTTP API (Phase 1.4).
    Vault,
    /// AWS Secrets Manager (Phase 1.4).
    AwsSecretsManager,
    /// Kubernetes Secret resource via API (Phase 1.4).
    K8sSecret,
}

pub(crate) fn load() -> Result<SecretsRuntimeConfig, RuntimeConfigError> {
    let provider = parse_provider(env_string("SECRETS_PROVIDER").as_deref())?;
    let vault_url = env_string("SECRETS_VAULT_URL");
    let vault_token = env_string("SECRETS_VAULT_TOKEN")
        .map(|v| SecretRef::parse("HIGHPER_SECRETS_VAULT_TOKEN", &v))
        .transpose()?;
    let aws_region = env_string("SECRETS_AWS_REGION");
    let k8s_namespace = env_string("SECRETS_K8S_NAMESPACE");

    let cache_ttl_secs = match env_string("SECRETS_CACHE_TTL") {
        None => 300,
        Some(s) => s
            .trim()
            .parse::<u64>()
            .map_err(|_| RuntimeConfigError::ParseError {
                env_var: "HIGHPER_SECRETS_CACHE_TTL".into(),
                value: s,
                expected: "u64 seconds",
            })?,
    };

    let cfg = SecretsRuntimeConfig {
        provider,
        vault_url,
        vault_token,
        aws_region,
        k8s_namespace,
        cache_ttl_secs: Reloadable::new(cache_ttl_secs),
    };
    validate_provider_requirements(&cfg)?;
    Ok(cfg)
}

fn validate_provider_requirements(cfg: &SecretsRuntimeConfig) -> Result<(), RuntimeConfigError> {
    match cfg.provider {
        SecretsProvider::Vault => {
            if cfg.vault_url.is_none() {
                return Err(RuntimeConfigError::MissingRequired {
                    env_var: "HIGHPER_SECRETS_VAULT_URL".into(),
                    required_because: "provider=vault",
                });
            }
            if cfg.vault_token.is_none() {
                return Err(RuntimeConfigError::MissingRequired {
                    env_var: "HIGHPER_SECRETS_VAULT_TOKEN".into(),
                    required_because: "provider=vault",
                });
            }
        }
        SecretsProvider::AwsSecretsManager => {
            if cfg.aws_region.is_none() {
                return Err(RuntimeConfigError::MissingRequired {
                    env_var: "HIGHPER_SECRETS_AWS_REGION".into(),
                    required_because: "provider=aws",
                });
            }
        }
        SecretsProvider::K8sSecret => {
            if cfg.k8s_namespace.is_none() {
                return Err(RuntimeConfigError::MissingRequired {
                    env_var: "HIGHPER_SECRETS_K8S_NAMESPACE".into(),
                    required_because: "provider=k8s",
                });
            }
        }
        SecretsProvider::Env | SecretsProvider::File => {}
    }
    Ok(())
}

fn parse_provider(v: Option<&str>) -> Result<SecretsProvider, RuntimeConfigError> {
    match v {
        None | Some("env") => Ok(SecretsProvider::Env),
        Some("file") => Ok(SecretsProvider::File),
        Some("vault") => Ok(SecretsProvider::Vault),
        Some("aws") => Ok(SecretsProvider::AwsSecretsManager),
        Some("k8s") => Ok(SecretsProvider::K8sSecret),
        Some(other) => Err(RuntimeConfigError::ParseError {
            env_var: "HIGHPER_SECRETS_PROVIDER".into(),
            value: other.into(),
            expected: "env|file|vault|aws|k8s",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear() {
        for k in [
            "HIGHPER_SECRETS_PROVIDER",
            "HIGHPER_SECRETS_VAULT_URL",
            "HIGHPER_SECRETS_VAULT_TOKEN",
            "HIGHPER_SECRETS_AWS_REGION",
            "HIGHPER_SECRETS_K8S_NAMESPACE",
            "HIGHPER_SECRETS_CACHE_TTL",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn default_provider_env() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(cfg.provider, SecretsProvider::Env);
    }

    #[test]
    #[serial]
    fn vault_provider_requires_url() {
        clear();
        std::env::set_var("HIGHPER_SECRETS_PROVIDER", "vault");
        let r = load();
        assert!(matches!(
            r,
            Err(RuntimeConfigError::MissingRequired { ref env_var, .. })
                if env_var == "HIGHPER_SECRETS_VAULT_URL"
        ));
        clear();
    }

    #[test]
    #[serial]
    fn vault_provider_requires_token() {
        clear();
        std::env::set_var("HIGHPER_SECRETS_PROVIDER", "vault");
        std::env::set_var("HIGHPER_SECRETS_VAULT_URL", "https://vault.local:8200");
        let r = load();
        assert!(matches!(
            r,
            Err(RuntimeConfigError::MissingRequired { ref env_var, .. })
                if env_var == "HIGHPER_SECRETS_VAULT_TOKEN"
        ));
        clear();
    }

    #[test]
    #[serial]
    fn aws_provider_requires_region() {
        clear();
        std::env::set_var("HIGHPER_SECRETS_PROVIDER", "aws");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::MissingRequired { .. })));
        clear();
    }

    #[test]
    #[serial]
    fn k8s_provider_requires_namespace() {
        clear();
        std::env::set_var("HIGHPER_SECRETS_PROVIDER", "k8s");
        let r = load();
        assert!(matches!(r, Err(RuntimeConfigError::MissingRequired { .. })));
        clear();
    }
}
