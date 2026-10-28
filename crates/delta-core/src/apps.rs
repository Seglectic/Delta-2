//! Apps for pickers: what's running (from KWin's window list) and what's installed
//! (.desktop files), with the launch command and match strings Delta needs.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// KWin script that reports open windows to deltad (`ReportWindows`), then is unloaded.
pub const LIST_WINDOWS_SCRIPT: &str = r#"var out = [];
var windows = workspace.windowList();
for (var i = 0; i < windows.length; ++i) {
    var w = windows[i];
    if (!w.normalWindow || w.skipTaskbar) continue;
    out.push({
        class: "" + w.resourceClass,
        desktop: "" + (w.desktopFileName || ""),
        caption: "" + w.caption,
        pid: w.pid || 0
    });
}
callDBus("org.seglectic.Delta", "/org/seglectic/Delta", "org.seglectic.Delta1", "ReportWindows", JSON.stringify(out));
"#;

/// One window as the KWin script reports it.
#[derive(Debug, Clone, Deserialize)]
pub struct Window {
    pub class: String,
    pub desktop: String,
    pub caption: String,
    pub pid: u32,
}

/// An app ready to drop into `[[apps]]`, or to use as an `only_in` class.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppChoice {
    /// Display name, e.g. `Dolphin`.
    pub label: String,
    /// Suggested `[[apps]]` name, e.g. `dolphin`.
    pub name: String,
    /// Window class, what `only_in` matches, e.g. `org.kde.dolphin`.
    pub class: String,
    /// Launch command (from the desktop file, else the process command line).
    pub launch: String,
    /// Strings that identify its windows (desktop ID and class), lower-case.
    #[serde(rename = "match")]
    pub match_: Vec<String>,
    /// A window title, to tell apps apart (running apps only).
    pub caption: Option<String>,
}

/// The parts of a .desktop file Delta uses.
#[derive(Debug, Clone, PartialEq)]
pub struct DesktopEntry {
    /// Desktop ID: the file name without `.desktop`.
    pub id: String,
    pub name: String,
    pub exec: String,
    pub wm_class: Option<String>,
    pub hidden: bool,
}

/// Where .desktop files live, user ones first.
pub fn desktop_dirs() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let data_home = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
    let mut dirs = vec![data_home.join("applications"), data_home.join("flatpak/exports/share/applications")];
    let system = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(system.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("applications")));
    dirs.push("/var/lib/flatpak/exports/share/applications".into());
    dirs.dedup();
    dirs
}

/// Parse the `[Desktop Entry]` group of a .desktop file. None if it isn't an app.
pub fn parse_desktop(id: &str, text: &str) -> Option<DesktopEntry> {
    let mut in_entry = false;
    let (mut name, mut exec, mut wm_class, mut kind) = (None, None, None, None);
    let mut hidden = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        let Some((key, value)) = line.split_once('=').filter(|_| in_entry) else { continue };
        let value = value.trim().to_string();
        match key.trim() {
            "Name" => name = Some(value),
            "Exec" => exec = Some(value),
            "StartupWMClass" => wm_class = Some(value),
            "Type" => kind = Some(value),
            "NoDisplay" | "Hidden" if value == "true" => hidden = true,
            _ => {}
        }
    }
    if kind.as_deref() != Some("Application") {
        return None;
    }
    Some(DesktopEntry { id: id.into(), name: name.unwrap_or_else(|| id.into()), exec: exec?, wm_class, hidden })
}

/// Find a desktop entry by ID in the standard directories.
pub fn find_desktop(id: &str) -> Option<DesktopEntry> {
    let id = id.strip_suffix(".desktop").unwrap_or(id);
    desktop_dirs()
        .into_iter()
        .find_map(|dir| std::fs::read_to_string(dir.join(format!("{id}.desktop"))).ok())
        .and_then(|text| parse_desktop(id, &text))
}

/// Every desktop entry that's an app, first one per ID (user dirs win).
fn all_desktop_entries() -> Vec<DesktopEntry> {
    let mut out: Vec<DesktopEntry> = Vec::new();
    for dir in desktop_dirs() {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let Some(id) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".desktop")) else {
                continue;
            };
            if out.iter().any(|d| d.id == id) {
                continue;
            }
            if let Some(d) = std::fs::read_to_string(&path).ok().and_then(|t| parse_desktop(id, &t)) {
                out.push(d);
            }
        }
    }
    out
}

/// Every installed app (one per window class), for the `only_in` picker.
pub fn installed_apps() -> Vec<AppChoice> {
    let mut out: Vec<AppChoice> = Vec::new();
    for d in all_desktop_entries().into_iter().filter(|d| !d.hidden) {
        let class = d.wm_class.clone().unwrap_or_else(|| d.id.clone());
        if !out.iter().any(|a| a.class.eq_ignore_ascii_case(&class)) {
            out.push(choice(&d, &class, None));
        }
    }
    out.sort_by_key(|a| a.label.to_lowercase());
    out
}

/// Window classes that are Delta's own windows, never offered as apps.
const OWN_CLASSES: &[&str] = &["delta-app"];

/// Running apps from KWin's window list, one per app, with launch commands.
/// The desktop file is found by, in order: what KWin reports, a desktop ID or
/// `StartupWMClass` equal to the window class, then the Flatpak app the process
/// belongs to. Without one, the process's own command line is used.
pub fn running_apps(windows: &[Window]) -> Vec<AppChoice> {
    let installed = all_desktop_entries();
    let by_class = |class: &str| {
        installed.iter().find(|d| {
            d.id.eq_ignore_ascii_case(class) || d.wm_class.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(class))
        })
    };
    let mut out: Vec<AppChoice> = Vec::new();
    for w in windows.iter().filter(|w| !OWN_CLASSES.contains(&w.class.as_str())) {
        let flatpak = flatpak_app(w.pid);
        let entry = (!w.desktop.is_empty())
            .then(|| find_desktop(&w.desktop))
            .flatten()
            .or_else(|| by_class(&w.class).cloned())
            .or_else(|| flatpak.as_deref().and_then(find_desktop));
        let app = match &entry {
            Some(d) => choice(d, &w.class, Some(&w.caption)),
            None if steam_app_id(&w.class).is_some() => steam_choice(&w.class, &w.caption),
            None => {
                let launch = match &flatpak {
                    Some(id) => format!("flatpak run {id}"),
                    None => process_command(w.pid).unwrap_or_else(|| w.class.clone()),
                };
                let name = short_name(&w.class);
                AppChoice {
                    label: w.class.clone(),
                    name,
                    class: w.class.clone(),
                    launch,
                    match_: vec![w.class.to_lowercase()],
                    caption: Some(w.caption.clone()),
                }
            }
        };
        if !out.iter().any(|a| a.class == app.class) {
            out.push(app);
        }
    }
    out.sort_by_key(|a| a.label.to_lowercase());
    out
}

fn choice(d: &DesktopEntry, class: &str, caption: Option<&str>) -> AppChoice {
    let mut match_ = vec![d.id.to_lowercase()];
    if !match_.contains(&class.to_lowercase()) {
        match_.push(class.to_lowercase());
    }
    AppChoice {
        label: d.name.clone(),
        name: short_name(&d.id),
        class: class.into(),
        launch: strip_field_codes(&d.exec),
        match_,
        caption: caption.map(String::from),
    }
}

/// The Steam app ID in a game window's class, `steam_app_2357570` → `2357570`.
fn steam_app_id(class: &str) -> Option<&str> {
    class.strip_prefix("steam_app_").filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
}

/// A Steam game, named from its library manifest (`Overwatch®` → `overwatch`).
fn steam_choice(class: &str, caption: &str) -> AppChoice {
    let id = steam_app_id(class).unwrap_or_default();
    let label = steam_game_name(id).unwrap_or_else(|| format!("Steam game {id}"));
    let name: String = label.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    AppChoice {
        label: format!("{label} (Steam)"),
        name: if name.is_empty() { format!("steam{id}") } else { name },
        class: class.into(),
        launch: format!("steam steam://rungameid/{id}"),
        match_: vec![class.to_lowercase()],
        caption: Some(caption.into()),
    }
}

/// The game's name from `appmanifest_<id>.acf` in any Steam library.
fn steam_game_name(id: &str) -> Option<String> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    let steam = [".local/share/Steam", ".steam/steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam"]
        .map(|p| home.join(p));
    let mut libraries: Vec<std::path::PathBuf> = steam.to_vec();
    for root in &steam {
        if let Ok(vdf) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            libraries.extend(vdf.lines().filter_map(|l| vdf_value(l, "path")).map(std::path::PathBuf::from));
        }
    }
    libraries.iter().find_map(|lib| {
        let acf = std::fs::read_to_string(lib.join(format!("steamapps/appmanifest_{id}.acf"))).ok()?;
        acf.lines().find_map(|l| vdf_value(l, "name")).filter(|n| !n.is_empty())
    })
}

/// `"key"\t\t"value"` → `value`, for one line of a Valve KeyValues file.
fn vdf_value(line: &str, key: &str) -> Option<String> {
    let rest = line.trim().strip_prefix(&format!("\"{key}\""))?.trim();
    Some(rest.strip_prefix('"')?.strip_suffix('"')?.replace("\\\\", "\\"))
}

/// `org.kde.dolphin` → `dolphin`, `com.discordapp.Discord` → `discord`.
fn short_name(id: &str) -> String {
    let last = id.rsplit('.').next().unwrap_or(id).to_lowercase();
    let clean: String = last.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')).collect();
    if clean.is_empty() { "app".into() } else { clean }
}

/// Drop the placeholders in an `Exec=` line (`%u`, `%F`, …) and Flatpak's `@@`
/// markers; `%%` is a literal percent sign.
pub fn strip_field_codes(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|w| !is_field_code(w) && !w.starts_with("@@"))
        .map(|w| w.replace("%%", "%"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `%u`, `%F` and friends (but not `%%`).
fn is_field_code(word: &str) -> bool {
    word.len() == 2 && word.starts_with('%') && word != "%%"
}

/// The Flatpak app ID a process runs in, from its cgroup
/// (`…/app-flatpak-com.discordapp.Discord-12345.scope`).
fn flatpak_app(pid: u32) -> Option<String> {
    let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    flatpak_app_from_cgroup(&cgroup)
}

fn flatpak_app_from_cgroup(cgroup: &str) -> Option<String> {
    let scope = cgroup.lines().find_map(|l| l.rsplit('/').next()?.strip_prefix("app-flatpak-"))?;
    let scope = scope.strip_suffix(".scope")?;
    let (id, pid) = scope.rsplit_once('-')?;
    pid.chars().all(|c| c.is_ascii_digit()).then(|| id.to_string())
}

/// A process's command line, shell-quoted, from /proc.
fn process_command(pid: u32) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let args: Vec<String> =
        raw.split(|b| *b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    (!args.is_empty()).then(|| shell_words::join(args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_exec_placeholders() {
        assert_eq!(strip_field_codes("dolphin %u"), "dolphin");
        assert_eq!(
            strip_field_codes(
                "/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=com.discordapp.Discord com.discordapp.Discord @@u %U @@"
            ),
            "/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=com.discordapp.Discord com.discordapp.Discord"
        );
        assert_eq!(strip_field_codes("app --rate 100%%"), "app --rate 100%");
    }

    #[test]
    fn reads_steam_manifest_lines() {
        assert_eq!(steam_app_id("steam_app_2357570"), Some("2357570"));
        assert_eq!(steam_app_id("steam_app_"), None);
        assert_eq!(vdf_value("\t\"name\"\t\t\"Overwatch®\"", "name").as_deref(), Some("Overwatch®"));
        assert_eq!(vdf_value("\t\"appid\"\t\t\"2357570\"", "name"), None);
    }

    #[test]
    fn parses_only_the_desktop_entry_group() {
        let text = "[Desktop Entry]\nType=Application\nName=Dolphin\nExec=dolphin %u\n\n[Desktop Action new]\nName=New Window\nExec=dolphin --new-window\n";
        let d = parse_desktop("org.kde.dolphin", text).unwrap();
        assert_eq!((d.name.as_str(), d.exec.as_str()), ("Dolphin", "dolphin %u"));
        let c = choice(&d, "org.kde.dolphin", None);
        assert_eq!((c.name.as_str(), c.launch.as_str()), ("dolphin", "dolphin"));
        assert_eq!(c.match_, ["org.kde.dolphin"]);
        assert!(parse_desktop("x", "[Desktop Entry]\nType=Link\nURL=https://x\n").is_none());
    }

    #[test]
    fn flatpak_from_cgroup() {
        let cg = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-flatpak-com.discordapp.Discord-48213.scope\n";
        assert_eq!(flatpak_app_from_cgroup(cg).as_deref(), Some("com.discordapp.Discord"));
        assert_eq!(flatpak_app_from_cgroup("0::/user.slice/session-2.scope\n"), None);
    }

    #[test]
    fn short_names() {
        assert_eq!(short_name("com.discordapp.Discord"), "discord");
        assert_eq!(short_name("firefox"), "firefox");
    }
}
