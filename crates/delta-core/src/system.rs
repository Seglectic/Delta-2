//! The side-effecting half of Delta: write generated files, restart xremap,
//! and (re)load the KWin script.

use crate::config::Config;
use crate::generate;
use crate::paths::Paths;
use std::path::{Path, PathBuf};
use std::process::Command;

const KWIN_SCRIPT_NAME: &str = "delta-tile";

#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("writing {path}: {source}")]
    Write { path: String, source: std::io::Error },
    #[error("running {cmd}: {detail}")]
    Command { cmd: String, detail: String },
    #[error("KWin D-Bus: {0}")]
    Kwin(#[from] zbus::Error),
    #[error(
        "{user} isn't in the 'input' group in this session, so xremap can't read keyboards.\n\
         Add it with `sudo usermod -aG input {user}`, then log out and back in."
    )]
    NotInInputGroup { user: String },
}

/// One generated file and where it goes.
pub struct Artifact {
    pub path: PathBuf,
    pub contents: String,
}

/// Everything `apply` would write. `launcher_exec` is the `delta` binary that
/// `delta-launch@.service` runs as `<launcher_exec> launch %i`.
pub fn render(config: &Config, paths: &Paths, launcher_exec: &Path) -> Vec<Artifact> {
    let xremap_bin =
        bundled_xremap().or_else(|| find_in_path("xremap")).unwrap_or_else(|| home().join(".local/bin/xremap"));
    let launch = format!("{} launch", launcher_exec.display());
    vec![
        Artifact {
            path: paths.xremap_config.clone(),
            contents: generate::xremap_config(config, &launcher_exec.display().to_string()),
        },
        Artifact {
            path: paths.xremap_service.clone(),
            contents: generate::xremap_service(
                config,
                &xremap_bin.display().to_string(),
                &paths.xremap_config.display().to_string(),
            ),
        },
        Artifact { path: paths.launcher_unit.clone(), contents: generate::launcher_unit(&launch) },
        Artifact { path: paths.kwin_script.clone(), contents: generate::kwin_script(config) },
    ]
}

/// Writes artifacts whose contents changed. Returns the paths it wrote.
pub fn write_artifacts(artifacts: &[Artifact]) -> Result<Vec<PathBuf>, SystemError> {
    let mut written = Vec::new();
    for a in artifacts {
        if std::fs::read_to_string(&a.path).is_ok_and(|old| old == a.contents) {
            continue;
        }
        let err = |source| SystemError::Write { path: a.path.display().to_string(), source };
        if let Some(dir) = a.path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        std::fs::write(&a.path, &a.contents).map_err(err)?;
        written.push(a.path.clone());
    }
    Ok(written)
}

/// What `apply` did, for the CLI/GUI to report.
pub struct ApplyReport {
    pub written: Vec<PathBuf>,
    pub restarted_xremap: bool,
}

/// Write everything, then restart xremap (if its files changed or it isn't running)
/// and reload the KWin script.
pub async fn apply(config: &Config, paths: &Paths, launcher_exec: &Path) -> Result<ApplyReport, SystemError> {
    check_input_group()?;
    let artifacts = render(config, paths, launcher_exec);
    let written = write_artifacts(&artifacts)?;

    let xremap_changed = written.iter().any(|p| *p == paths.xremap_config || *p == paths.xremap_service);
    let unit_changed = written.iter().any(|p| *p == paths.xremap_service || *p == paths.launcher_unit);
    if unit_changed {
        systemctl(&["daemon-reload"])?;
    }
    let restart = xremap_changed || xremap_state() != "active";
    if restart {
        // Enabling fails harmlessly if it already is; restart must succeed
        let _ = systemctl(&["enable", "xremap"]);
        systemctl(&["restart", "xremap"])?;
    }
    reload_kwin(&paths.kwin_script).await?;
    Ok(ApplyReport { written, restarted_xremap: restart })
}

/// Restart xremap and reload the KWin script without regenerating anything.
pub async fn reload(paths: &Paths) -> Result<(), SystemError> {
    systemctl(&["restart", "xremap"])?;
    reload_kwin(&paths.kwin_script).await
}

/// Unload and load the KWin script so registered shortcuts pick up changes.
pub async fn reload_kwin(script: &Path) -> Result<(), SystemError> {
    let conn = zbus::Connection::session().await?;
    // Unloading a script that isn't loaded just returns false
    kwin_call(&conn, "unloadScript", &(KWIN_SCRIPT_NAME,)).await?;
    kwin_call(&conn, "loadScript", &(script.display().to_string(), KWIN_SCRIPT_NAME)).await?;
    kwin_call(&conn, "start", &()).await?;
    Ok(())
}

/// Load and start a KWin script under `name`, replacing one already loaded with it.
pub async fn run_kwin_script(script: &Path, name: &str) -> Result<(), SystemError> {
    let conn = zbus::Connection::session().await?;
    kwin_call(&conn, "unloadScript", &(name,)).await?;
    kwin_call(&conn, "loadScript", &(script.display().to_string(), name)).await?;
    kwin_call(&conn, "start", &()).await?;
    Ok(())
}

pub async fn unload_kwin_script(name: &str) -> Result<(), SystemError> {
    let conn = zbus::Connection::session().await?;
    kwin_call(&conn, "unloadScript", &(name,)).await?;
    Ok(())
}

/// Whether KWin currently has the delta script loaded.
pub async fn kwin_script_loaded() -> Result<bool, SystemError> {
    let conn = zbus::Connection::session().await?;
    let reply = kwin_call(&conn, "isScriptLoaded", &(KWIN_SCRIPT_NAME,)).await?;
    Ok(reply.body().deserialize::<bool>()?)
}

async fn kwin_call<B>(conn: &zbus::Connection, method: &str, body: &B) -> zbus::Result<zbus::Message>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
{
    conn.call_method(Some("org.kde.KWin"), "/Scripting", Some("org.kde.kwin.Scripting"), method, body).await
}

/// Fire a KDE global shortcut by name, exactly as if its keys were pressed. Delta's
/// window actions are registered under the `kwin` component by their binding ID.
pub async fn invoke_shortcut(component: &str, name: &str) -> Result<(), SystemError> {
    let conn = zbus::Connection::session().await?;
    conn.call_method(
        Some("org.kde.kglobalaccel"),
        format!("/component/{component}").as_str(),
        Some("org.kde.kglobalaccel.Component"),
        "invokeShortcut",
        &(name,),
    )
    .await?;
    Ok(())
}

/// A KDE global shortcut, for pickers.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Shortcut {
    /// kglobalaccel component, e.g. `kwin`, `plasmashell`.
    pub component: String,
    /// The component's display name, e.g. `KWin`.
    pub component_name: String,
    /// Internal name, what `kwin_shortcut` actions use.
    pub name: String,
    /// Display name, e.g. `Quick Tile Window to the Top Right`.
    pub label: String,
}

/// Every KDE global shortcut, except the ones Delta registers itself.
pub async fn list_shortcuts() -> Result<Vec<Shortcut>, SystemError> {
    type Info = (String, String, String, String, String, String, Vec<i32>, Vec<i32>);
    let conn = zbus::Connection::session().await?;
    let reply = conn
        .call_method(Some("org.kde.kglobalaccel"), "/kglobalaccel", Some("org.kde.KGlobalAccel"), "allComponents", &())
        .await?;
    let components: Vec<zbus::OwnedObjectPath> = reply.body().deserialize()?;
    let mut out = Vec::new();
    for path in components {
        let reply = conn
            .call_method(
                Some("org.kde.kglobalaccel"),
                path.as_str(),
                Some("org.kde.kglobalaccel.Component"),
                "allShortcutInfos",
                &(),
            )
            .await?;
        let infos: Vec<Info> = reply.body().deserialize()?;
        for (name, label, component, component_name, ..) in infos {
            if label.starts_with("Delta: ") {
                continue;
            }
            let label = if label.is_empty() { name.clone() } else { label };
            out.push(Shortcut { component, component_name, name, label });
        }
    }
    out.sort_by(|a, b| a.component_name.cmp(&b.component_name).then(a.label.cmp(&b.label)));
    Ok(out)
}

/// `systemctl --user is-active xremap`: "active", "inactive", "failed", …
pub fn xremap_state() -> String {
    Command::new("systemctl")
        .args(["--user", "is-active", "xremap"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

pub(crate) fn systemctl(args: &[&str]) -> Result<(), SystemError> {
    let cmd = format!("systemctl --user {}", args.join(" "));
    let out = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map_err(|e| SystemError::Command { cmd: cmd.clone(), detail: e.to_string() })?;
    if out.status.success() {
        Ok(())
    } else {
        Err(SystemError::Command { cmd, detail: String::from_utf8_lossy(&out.stderr).trim().to_string() })
    }
}

/// xremap reads /dev/input, which needs the input group in the *current* session.
fn check_input_group() -> Result<(), SystemError> {
    let groups = Command::new("id").arg("-nG").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    match groups {
        Ok(g) if g.split_whitespace().any(|g| g == "input") => Ok(()),
        _ => Err(SystemError::NotInInputGroup { user: std::env::var("USER").unwrap_or_else(|_| "you".into()) }),
    }
}

/// The xremap installed next to the running binary (the released app ships one).
fn bundled_xremap() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    Some(exe.parent()?.join("xremap")).filter(|p| p.is_file())
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}

fn find_in_path(bin: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|dir| dir.join(bin)).find(|p| p.is_file())
}
