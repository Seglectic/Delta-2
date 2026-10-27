//! Where Delta reads its config and writes generated files.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub xremap_config: PathBuf,
    pub xremap_service: PathBuf,
    pub launcher_unit: PathBuf,
    pub kwin_script: PathBuf,
    /// Your own VIA keyboard definitions (`*.json`); these win over fetched ones.
    pub layouts: PathBuf,
    /// VIA definitions fetched from usevia.app, by USB ID.
    pub layout_cache: PathBuf,
    /// Keymap backups (`delta keyboard backup`).
    pub backups: PathBuf,
    /// The legacy bash install, used as the default `delta import` source.
    pub legacy_conf: PathBuf,
}

impl Paths {
    /// Standard locations under `$HOME` / `$XDG_CONFIG_HOME`.
    pub fn from_env() -> Self {
        let home = env_path("HOME").unwrap_or_else(|| "/".into());
        let config_home = env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
        let data_home = env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
        let cache_home = env_path("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache"));
        Self {
            config: config_home.join("delta/delta.toml"),
            xremap_config: config_home.join("xremap/config.yml"),
            xremap_service: config_home.join("systemd/user/xremap.service"),
            launcher_unit: config_home.join("systemd/user/delta-launch@.service"),
            kwin_script: data_home.join("kwin/scripts/delta-tile/contents/code/main.js"),
            layouts: config_home.join("delta/layouts"),
            layout_cache: cache_home.join("delta/layouts"),
            backups: data_home.join("delta/backups"),
            legacy_conf: home.join(".local/bin/delta/delta.conf"),
        }
    }
}

/// An environment variable as a path. Empty or relative values count as unset (XDG spec).
fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute())
}
