//! `delta`: manage the Delta hotkey system from the terminal.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use delta_core::paths::Paths;
use delta_core::{Config, import, launch, system};
use std::path::{Path, PathBuf};

mod keyboard;

#[derive(Parser)]
#[command(name = "delta", version, about = "Caps Lock layer hotkeys for KDE Plasma")]
struct Cli {
    /// Config file (default: ~/.config/delta/delta.toml)
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Convert a legacy delta.conf into delta.toml
    Import {
        /// Legacy config (default: ~/.local/bin/delta/delta.conf)
        #[arg(long)]
        from: Option<PathBuf>,
        /// Overwrite an existing delta.toml
        #[arg(long)]
        force: bool,
    },
    /// Validate the config
    Check,
    /// Show what `apply` would change on this system, without changing it
    Diff,
    /// Write every generated file into a directory instead of the live locations
    Render {
        #[arg(long)]
        out: PathBuf,
    },
    /// Regenerate xremap config, KWin script and units, then reload them
    Apply,
    /// Restart xremap and reload the KWin script without regenerating
    Reload,
    /// Reload only the KWin script
    ReloadKwin,
    /// Show the state of every component
    Status,
    /// Launch an app from the config (used by delta-launch@.service)
    Launch { app: String },
    /// Fire a KDE global shortcut by name, e.g. `delta shortcut Overview` (`list` lists them)
    Shortcut {
        name: String,
        /// kglobalaccel component that owns the shortcut
        #[arg(long, default_value = "kwin")]
        component: String,
    },
    /// Read connected QMK keyboards over VIA (read-only)
    #[command(subcommand)]
    Keyboard(KeyboardCmd),
}

#[derive(Subcommand)]
enum KeyboardCmd {
    /// List keyboards with a VIA interface
    List,
    /// Draw each layer of a keyboard's keymap as the firmware has it
    Dump {
        /// Only this layer
        #[arg(long)]
        layer: Option<u8>,
    },
    /// Firmware, layers and lighting details (read-only)
    Info,
    /// Save the whole keymap (layers, knobs, layout options, macros) to JSON, read-only
    Backup {
        /// Output file (default: ~/.local/share/delta/backups/<vid>_<pid>-<date>.json)
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut paths = Paths::from_env();
    if let Some(config) = cli.config {
        paths.config = config;
    }

    match cli.command {
        Cmd::Import { from, force } => cmd_import(&paths, from, force),
        Cmd::Check => {
            load(&paths)?;
            println!("{} is valid", paths.config.display());
            Ok(())
        }
        Cmd::Diff => cmd_diff(&paths),
        Cmd::Render { out } => cmd_render(&paths, &out),
        Cmd::Apply => cmd_apply(&paths),
        Cmd::Reload => {
            zbus::block_on(system::reload(&paths))?;
            println!("Reloaded xremap and the KWin script.");
            Ok(())
        }
        Cmd::ReloadKwin => {
            zbus::block_on(system::reload_kwin(&paths.kwin_script))?;
            println!("Reloaded the KWin script.");
            Ok(())
        }
        Cmd::Status => cmd_status(&paths),
        Cmd::Keyboard(KeyboardCmd::List) => keyboard::list(&paths),
        Cmd::Keyboard(KeyboardCmd::Dump { layer }) => keyboard::dump(&paths, layer),
        Cmd::Keyboard(KeyboardCmd::Backup { out }) => keyboard::backup(&paths, out),
        Cmd::Keyboard(KeyboardCmd::Info) => keyboard::info(&paths),
        Cmd::Shortcut { name, component } if name == "list" => {
            for s in zbus::block_on(system::list_shortcuts())? {
                println!("{:<14} {:<40} {}", s.component, s.name, s.label);
            }
            Ok(())
        }
        Cmd::Shortcut { name, component } => Ok(zbus::block_on(system::invoke_shortcut(&component, &name))?),
        Cmd::Launch { app } => {
            let config = load(&paths)?;
            Err(launch::exec_app(&config, &app)).with_context(|| format!("launching {app}"))
        }
    }
}

fn load(paths: &Paths) -> Result<Config> {
    if !paths.config.exists() {
        bail!("{} doesn't exist yet. Run `delta import` to create it from delta.conf.", paths.config.display());
    }
    Ok(Config::load(&paths.config)?)
}

/// The binary `delta-launch@.service` should run.
fn launcher_exec() -> Result<PathBuf> {
    std::env::current_exe()?.canonicalize().context("finding the delta binary")
}

fn cmd_import(paths: &Paths, from: Option<PathBuf>, force: bool) -> Result<()> {
    let from = from.unwrap_or_else(|| paths.legacy_conf.clone());
    if paths.config.exists() && !force {
        bail!("{} already exists (use --force to overwrite)", paths.config.display());
    }
    let text = std::fs::read_to_string(&from).with_context(|| format!("reading {}", from.display()))?;
    let result = import::import_conf(&text);
    for w in &result.warnings {
        eprintln!("warning: {w}");
    }
    if let Err(problems) = result.config.validate() {
        bail!("the imported config isn't valid:\n{problems}");
    }

    let header = format!(
        "# Delta config. Imported from {} on first run.\n\
         # Edit this file or use the Delta app, then run `delta apply`.\n\n",
        from.display()
    );
    if let Some(dir) = paths.config.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&paths.config, header + &result.config.to_toml())?;
    println!(
        "Wrote {} ({} bindings, {} apps).",
        paths.config.display(),
        result.config.bindings.len(),
        result.config.apps.len()
    );
    Ok(())
}

fn cmd_diff(paths: &Paths) -> Result<()> {
    let config = load(paths)?;
    let mut changes = 0;
    for a in system::render(&config, paths, &launcher_exec()?) {
        match std::fs::read_to_string(&a.path) {
            Ok(old) if old == a.contents => println!("  unchanged  {}", a.path.display()),
            Ok(old) => {
                changes += 1;
                println!("  changed    {}", a.path.display());
                for line in line_diff(&old, &a.contents) {
                    println!("             {line}");
                }
            }
            Err(_) => {
                changes += 1;
                println!("  new        {}", a.path.display());
            }
        }
    }
    println!("{changes} file(s) would change.");
    Ok(())
}

/// Minimal "-old / +new" listing of differing lines, enough to eyeball a change.
fn line_diff(old: &str, new: &str) -> Vec<String> {
    let (old, new): (Vec<_>, Vec<_>) = (old.lines().collect(), new.lines().collect());
    let mut out = Vec::new();
    for i in 0..old.len().max(new.len()) {
        match (old.get(i), new.get(i)) {
            (Some(a), Some(b)) if a == b => {}
            (a, b) => {
                if let Some(a) = a {
                    out.push(format!("-{a}"));
                }
                if let Some(b) = b {
                    out.push(format!("+{b}"));
                }
            }
        }
    }
    out
}

fn cmd_render(paths: &Paths, out: &Path) -> Result<()> {
    let config = load(paths)?;
    for a in system::render(&config, paths, &launcher_exec()?) {
        let name = a.path.file_name().context("artifact path has no file name")?;
        let dest = out.join(name);
        std::fs::create_dir_all(out)?;
        std::fs::write(&dest, &a.contents)?;
        println!("wrote {}", dest.display());
    }
    Ok(())
}

fn cmd_apply(paths: &Paths) -> Result<()> {
    let config = load(paths)?;
    let report = zbus::block_on(system::apply(&config, paths, &launcher_exec()?))?;
    if report.written.is_empty() {
        println!("No generated files changed.");
    }
    for p in &report.written {
        println!("wrote {}", p.display());
    }
    if report.restarted_xremap {
        println!("Restarted xremap.");
    }
    println!("Reloaded the KWin script.");
    Ok(())
}

fn cmd_status(paths: &Paths) -> Result<()> {
    let kwin = match zbus::block_on(system::kwin_script_loaded()) {
        Ok(true) => "loaded".to_string(),
        Ok(false) => "not loaded".to_string(),
        Err(e) => format!("unknown ({e})"),
    };
    let valid = match Config::load(&paths.config) {
        Ok(c) => format!("valid, {} bindings", c.bindings.len()),
        Err(e) => format!("{e}").lines().next().unwrap_or_default().to_string(),
    };
    println!("config:           {} ({valid})", paths.config.display());
    println!("xremap service:   {}", system::xremap_state());
    println!("xremap config:    {}", paths.xremap_config.display());
    println!("kwin script:      {kwin}");
    println!("kwin script path: {}", paths.kwin_script.display());
    Ok(())
}
