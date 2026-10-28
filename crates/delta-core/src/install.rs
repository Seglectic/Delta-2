//! The released app installs itself: it copies the deltad, delta and xremap it ships
//! with out of the AppImage, points deltad.service at them, and checks the permissions
//! they need. A deltad.service written by `packaging/install.sh` (a dev build) is left
//! alone.

use crate::paths::Paths;
use crate::system::{self, SystemError};
use std::fs::OpenOptions;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Programs the app bundles, copied to `Paths::bin_dir`.
pub const BINARIES: [&str; 3] = ["deltad", "delta", "xremap"];

const UNIT_TEMPLATE: &str = include_str!("../../../packaging/deltad.service");

/// Written by `setup_command`, as root.
pub const UDEV_RULES: &str = "/etc/udev/rules.d/70-delta.rules";

/// Who runs deltad.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", content = "exec", rename_all = "snake_case")]
pub enum Owner {
    /// No deltad.service yet.
    Nobody,
    /// deltad.service runs `Paths::bin_dir`'s deltad: the app keeps it up to date.
    App,
    /// deltad.service runs something else, e.g. a dev build from install.sh.
    Other(String),
}

pub fn owner(paths: &Paths) -> Owner {
    let Ok(text) = std::fs::read_to_string(&paths.deltad_unit) else { return Owner::Nobody };
    match exec_start(&text) {
        Some(exec) if Path::new(exec) == paths.bin_dir.join("deltad") => Owner::App,
        Some(exec) => Owner::Other(exec.to_string()),
        None => Owner::Other(String::new()),
    }
}

fn exec_start(unit: &str) -> Option<&str> {
    unit.lines().find_map(|l| l.strip_prefix("ExecStart=")).map(str::trim)
}

/// deltad.service running the deltad in `bin_dir`.
pub fn unit(paths: &Paths) -> String {
    let exec = paths.bin_dir.join("deltad");
    UNIT_TEMPLATE
        .lines()
        .map(|l| if exec_start(l).is_some() { format!("ExecStart={}", exec.display()) } else { l.to_string() })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("{0} isn't bundled with the app")]
    Missing(String),
    #[error("installing {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error(transparent)]
    System(#[from] SystemError),
}

/// What `sync` did.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Sync {
    /// deltad.service belongs to someone else; nothing was touched.
    Skipped { exec: String },
    /// Files that changed (empty: already up to date) and whether deltad was restarted.
    Done { updated: Vec<String>, restarted: bool },
}

/// Install the programs in `from` (the app's own directory) into `bin_dir` where they
/// differ, write deltad.service and the `delta` link, and (re)start deltad if anything
/// changed or it isn't running.
pub fn sync(paths: &Paths, from: &Path) -> Result<Sync, InstallError> {
    if let Owner::Other(exec) = owner(paths) {
        return Ok(Sync::Skipped { exec });
    }
    let mut updated = Vec::new();
    for name in BINARIES {
        let src = from.join(name);
        let bytes = std::fs::read(&src).map_err(|_| InstallError::Missing(name.into()))?;
        let dst = paths.bin_dir.join(name);
        if write_if_changed(&dst, &bytes, 0o755)? {
            updated.push(name.to_string());
        }
    }
    let unit_changed = write_if_changed(&paths.deltad_unit, unit(paths).as_bytes(), 0o644)?;
    if unit_changed {
        updated.push("deltad.service".into());
    }
    link_cli(paths)?;

    let running = Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", "deltad"])
        .status()
        .is_ok_and(|s| s.success());
    let restart = !updated.is_empty() || !running;
    if unit_changed {
        system::systemctl(&["daemon-reload"])?;
    }
    if restart {
        system::systemctl(&["enable", "deltad"])?;
        system::systemctl(&["restart", "deltad"])?;
        // deltad restarts xremap when its service file changes, but not for a new
        // binary at the same path
        if updated.iter().any(|u| u == "xremap") {
            let _ = system::systemctl(&["try-restart", "xremap"]);
        }
    }
    Ok(Sync::Done { updated, restarted: restart })
}

/// Writes `bytes` to `path` (via a temp file, so a running binary can be replaced)
/// unless it already holds them. Returns whether it wrote.
fn write_if_changed(path: &Path, bytes: &[u8], mode: u32) -> Result<bool, InstallError> {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(false);
    }
    let err = |source| InstallError::Io { path: path.display().to_string(), source };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    let tmp = path.with_file_name(format!(".{}.tmp", path.file_name().unwrap_or_default().to_string_lossy()));
    std::fs::write(&tmp, bytes)
        .and_then(|()| std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode)))
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(err)?;
    Ok(true)
}

/// `~/.local/bin/delta` → `bin_dir/delta`, unless something that isn't a link is there
/// (install.sh's binary, or the old bash version's directory).
fn link_cli(paths: &Paths) -> Result<(), InstallError> {
    let target = paths.bin_dir.join("delta");
    let err = |source| InstallError::Io { path: paths.cli_link.display().to_string(), source };
    match std::fs::symlink_metadata(&paths.cli_link) {
        Ok(m) if !m.file_type().is_symlink() => return Ok(()),
        Ok(_) if std::fs::read_link(&paths.cli_link).is_ok_and(|t| t == target) => return Ok(()),
        Ok(_) => std::fs::remove_file(&paths.cli_link).map_err(err)?,
        Err(_) => {}
    }
    if let Some(dir) = paths.cli_link.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::os::unix::fs::symlink(&target, &paths.cli_link).map_err(err)
}

/// Menu entry that starts the AppImage at `appimage`, with the app's icon.
pub fn write_menu_entry(paths: &Paths, appimage: &Path, icon_png: &[u8]) -> Result<(), InstallError> {
    write_if_changed(&paths.icon, icon_png, 0o644)?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Delta\nComment=Keyboard and mouse hotkeys\n\
         Exec=\"{}\"\nIcon={}\nStartupWMClass=delta-app\nCategories=Settings;Utility;\nTerminal=false\n",
        appimage.display(),
        paths.icon.display()
    );
    write_if_changed(&paths.desktop_entry, entry.as_bytes(), 0o644)?;
    Ok(())
}

/// What Delta can and can't reach on this machine.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Permissions {
    /// In the `input` group in this login session (xremap reads /dev/input).
    pub input_session: bool,
    /// In the `input` group in /etc/group (maybe only after logging in again).
    pub input_configured: bool,
    /// Can open /dev/uinput (xremap's virtual keyboard).
    pub uinput: bool,
    /// Can open every /dev/hidraw* (VIA keyboards).
    pub hidraw: bool,
    /// Delta's udev rules are installed.
    pub rules: bool,
}

impl Permissions {
    pub fn check() -> Self {
        let user = std::env::var("USER").unwrap_or_default();
        let has_input = |args: &[&str]| {
            Command::new("id")
                .args(args)
                .output()
                .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().any(|g| g == "input"))
        };
        let hidraw = std::fs::read_dir("/dev")
            .map(|dir| {
                dir.flatten()
                    .filter(|e| e.file_name().to_string_lossy().starts_with("hidraw"))
                    .all(|e| can_open(&e.path()))
            })
            .unwrap_or(true);
        Self {
            input_session: has_input(&["-nG"]),
            input_configured: has_input(&["-nG", &user]),
            uinput: can_open(Path::new("/dev/uinput")),
            hidraw,
            rules: Path::new(UDEV_RULES).exists(),
        }
    }

    /// Something only root can fix: run `setup_command`.
    pub fn needs_setup(&self) -> bool {
        let devices_ok = self.uinput && self.hidraw;
        !self.input_configured || !(self.rules || devices_ok)
    }

    /// Setup is done, but this session started before it.
    pub fn needs_relogin(&self) -> bool {
        let ready = self.input_session && self.uinput && self.hidraw;
        !(self.needs_setup() || ready)
    }
}

/// Opening a device without reading or writing it has no effect on it.
fn can_open(path: &Path) -> bool {
    OpenOptions::new().read(true).write(true).open(path).is_ok()
}

/// Run as root by `setup_command`; `$1` is the user.
const SETUP_SCRIPT: &str = r#"set -e
cat > /etc/udev/rules.d/70-delta.rules <<'EOF'
# Delta: xremap sends keys through uinput; Delta talks to VIA keyboards over hidraw
KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", MODE="0660", TAG+="uaccess"
EOF
echo uinput > /etc/modules-load.d/delta-uinput.conf
modprobe uinput || true
usermod -aG input "$1"
udevadm control --reload-rules
udevadm trigger --subsystem-match=hidraw --subsystem-match=misc
"#;

/// The one root step (asks for a password through polkit): udev rules for uinput and
/// hidraw, load uinput at boot, and add the user to `input`.
pub fn setup_command() -> Command {
    let mut cmd = Command::new("pkexec");
    cmd.args(["sh", "-c", SETUP_SCRIPT, "delta-setup"]).arg(std::env::var("USER").unwrap_or_default());
    cmd
}

/// The app's own directory, where Tauri puts the bundled programs.
pub fn bundled_dir() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_in(dir: &Path) -> Paths {
        let mut p = Paths::from_env();
        p.bin_dir = dir.join("bin");
        p.deltad_unit = dir.join("deltad.service");
        p
    }

    #[test]
    fn unit_runs_bin_dir_deltad() {
        let p = paths_in(Path::new("/x"));
        let u = unit(&p);
        assert_eq!(exec_start(&u), Some("/x/bin/deltad"));
        assert!(u.contains("WantedBy=graphical-session.target"));
    }

    #[test]
    fn owner_reads_exec_start() {
        let dir = std::env::temp_dir().join(format!("delta-install-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = paths_in(&dir);
        assert_eq!(owner(&p), Owner::Nobody);
        std::fs::write(&p.deltad_unit, unit(&p)).unwrap();
        assert_eq!(owner(&p), Owner::App);
        std::fs::write(&p.deltad_unit, UNIT_TEMPLATE).unwrap();
        assert_eq!(owner(&p), Owner::Other("%h/.local/bin/deltad".into()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
