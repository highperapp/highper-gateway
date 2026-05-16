//! Geographic-LB tunables (UC15 P0; Workstream 0.J Stage 3).

use std::path::PathBuf;

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone, Default)]
pub struct GeoRuntimeConfig {
    pub provider: Reloadable<GeoProvider>, // HIGHPER_GEO_PROVIDER = maxmind|ip2location|none
    pub maxmind_db_path: Reloadable<Option<PathBuf>>, // HIGHPER_GEO_MAXMIND_DB_PATH
    pub ip2location_db_path: Reloadable<Option<PathBuf>>, // HIGHPER_GEO_IP2LOCATION_DB_PATH
    pub fallback_country: Reloadable<Option<String>>, // HIGHPER_GEO_FALLBACK_COUNTRY (e.g., "US")
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GeoProvider {
    #[default]
    None,
    Maxmind,
    Ip2location,
}

pub(crate) fn load() -> Result<GeoRuntimeConfig, RuntimeConfigError> {
    let provider = match env_string("GEO_PROVIDER").as_deref() {
        None | Some("none") => GeoProvider::None,
        Some("maxmind") => GeoProvider::Maxmind,
        Some("ip2location") => GeoProvider::Ip2location,
        Some(other) => {
            return Err(RuntimeConfigError::ParseError {
                env_var: "HIGHPER_GEO_PROVIDER".into(),
                value: other.into(),
                expected: "maxmind|ip2location|none",
            });
        }
    };

    let cfg = GeoRuntimeConfig {
        provider: Reloadable::new(provider),
        maxmind_db_path: Reloadable::new(env_string("GEO_MAXMIND_DB_PATH").map(PathBuf::from)),
        ip2location_db_path: Reloadable::new(
            env_string("GEO_IP2LOCATION_DB_PATH").map(PathBuf::from),
        ),
        fallback_country: Reloadable::new(env_string("GEO_FALLBACK_COUNTRY")),
    };
    validate(&cfg)?;
    Ok(cfg)
}

fn validate(cfg: &GeoRuntimeConfig) -> Result<(), RuntimeConfigError> {
    match cfg.provider.get() {
        GeoProvider::Maxmind if cfg.maxmind_db_path.get().is_none() => {
            Err(RuntimeConfigError::MissingRequired {
                env_var: "HIGHPER_GEO_MAXMIND_DB_PATH".into(),
                required_because: "provider=maxmind",
            })
        }
        GeoProvider::Ip2location if cfg.ip2location_db_path.get().is_none() => {
            Err(RuntimeConfigError::MissingRequired {
                env_var: "HIGHPER_GEO_IP2LOCATION_DB_PATH".into(),
                required_because: "provider=ip2location",
            })
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear() {
        for k in [
            "HIGHPER_GEO_PROVIDER",
            "HIGHPER_GEO_MAXMIND_DB_PATH",
            "HIGHPER_GEO_IP2LOCATION_DB_PATH",
            "HIGHPER_GEO_FALLBACK_COUNTRY",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    #[serial]
    fn default_none() {
        clear();
        let cfg = load().unwrap();
        assert_eq!(*cfg.provider.get(), GeoProvider::None);
    }

    #[test]
    #[serial]
    fn maxmind_requires_db_path() {
        clear();
        std::env::set_var("HIGHPER_GEO_PROVIDER", "maxmind");
        assert!(matches!(
            load(),
            Err(RuntimeConfigError::MissingRequired { .. })
        ));
        clear();
    }

    #[test]
    #[serial]
    fn ip2location_requires_db_path() {
        clear();
        std::env::set_var("HIGHPER_GEO_PROVIDER", "ip2location");
        assert!(matches!(
            load(),
            Err(RuntimeConfigError::MissingRequired { .. })
        ));
        clear();
    }
}
