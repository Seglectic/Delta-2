//! Delta's shared core: config model, legacy import, generators, and the system
//! side (paths, writing generated files, reloading xremap and KWin).

pub mod apps;
pub mod config;
pub mod generate;
pub mod import;
pub mod launch;
pub mod paths;
pub mod system;

pub use config::Config;
