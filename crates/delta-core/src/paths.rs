//! Where Delta reads its config and writes generated files.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub xremap_config: PathBuf,
    pub xremap_service: PathBuf,
    pub launcher_unit: PathBuf,
    pub kwin_script: PathBuf,
    /// The legacy bash install, used as the default `delta import` source.
    pub legacy_conf: PathBuf,
}

impl Paths {
    /// Standard locations under `$HOME` / `$XDG_CONFIG_HOME`.
    pub fn from_env() -> Self {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()));
        let config_home =
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
        let data_home =
            std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
        Self {
            config: config_home.join("delta/delta.toml"),
            xremap_config: config_home.join("xremap/config.yml"),
            xremap_service: config_home.join("systemd/user/xremap.service"),
            launcher_unit: config_home.join("systemd/user/delta-launch@.service"),
            kwin_script: data_home.join("kwin/scripts/delta-tile/contents/code/main.js"),
            legacy_conf: home.join(".local/bin/delta/delta.conf"),
        }
    }
}
