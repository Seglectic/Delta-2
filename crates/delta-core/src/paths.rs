//! Where Delta reads its config and writes generated files.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub xremap_config: PathBuf,
    pub xremap_service: PathBuf,
    pub launcher_unit: PathBuf,
    pub kwin_script: PathBuf,
    /// Extra VIA keyboard definitions (`*.json`) beyond the built-in ones.
    pub layouts: PathBuf,
    /// The legacy bash install, used as the default `delta import` source.
    pub legacy_conf: PathBuf,
}

impl Paths {
    /// Standard locations under `$HOME` / `$XDG_CONFIG_HOME`.
    pub fn from_env() -> Self {
        let home = env_path("HOME").unwrap_or_else(|| "/".into());
        let config_home = env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
        let data_home = env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
        Self {
            config: config_home.join("delta/delta.toml"),
            xremap_config: config_home.join("xremap/config.yml"),
            xremap_service: config_home.join("systemd/user/xremap.service"),
            launcher_unit: config_home.join("systemd/user/delta-launch@.service"),
            kwin_script: data_home.join("kwin/scripts/delta-tile/contents/code/main.js"),
            layouts: config_home.join("delta/layouts"),
            legacy_conf: home.join(".local/bin/delta/delta.conf"),
        }
    }
}

/// An environment variable as a path. Empty or relative values count as unset (XDG spec).
fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute())
}
