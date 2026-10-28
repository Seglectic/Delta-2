//! Getting layouts in bulk: every board in VIA's keyboards repo in one download, and
//! layout files people hand us (VIA JSON, compiled or KLE draft).
//!
//! VIA's definitions are GPL-3.0, so Delta (MIT) downloads them at runtime instead of
//! shipping them.

use crate::layout::{Definition, LayoutError};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};

/// the-via/keyboards as one tarball; definitions are under `<root>/v3/`.
const VIA_REPO_TARBALL: &str = "https://codeload.github.com/the-via/keyboards/tar.gz/refs/heads/master";

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("downloading VIA's layouts: {0}")]
    Download(String),
    #[error("reading VIA's layouts: {0}")]
    Archive(#[from] std::io::Error),
    #[error("{path}: {source}")]
    Write { path: String, source: std::io::Error },
    #[error(transparent)]
    Layout(#[from] LayoutError),
}

/// What `download_all` did.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Downloaded {
    /// New layouts saved to the cache.
    pub saved: usize,
    /// Already cached; left as they were.
    pub cached: usize,
    /// Definitions in the repo Delta couldn't read.
    pub unreadable: usize,
}

/// Fetch VIA's keyboards repo and cache every definition (only `vendor_id`'s if given)
/// as `cache_dir/<vid>_<pid>.json`, where `Definition::load` looks.
pub fn download_all(cache_dir: &Path, vendor_id: Option<u16>) -> Result<Downloaded, LibraryError> {
    let response = ureq::get(VIA_REPO_TARBALL).call().map_err(|e| LibraryError::Download(e.to_string()))?;
    let reader = response.into_body().into_reader();
    save_from_tarball(reader, cache_dir, vendor_id)
}

fn save_from_tarball(reader: impl Read, cache_dir: &Path, vendor_id: Option<u16>) -> Result<Downloaded, LibraryError> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    let mut done = Downloaded::default();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        // `<repo>-master/v3/<maker>/.../<board>.json`
        let in_v3 = path.components().nth(1).is_some_and(|c| c.as_os_str() == "v3");
        if !in_v3 || path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let mut json = String::new();
        if entry.read_to_string(&mut json).is_err() {
            done.unreadable += 1;
            continue;
        }
        let Ok(def) = Definition::parse(&json) else {
            done.unreadable += 1;
            continue;
        };
        if vendor_id.is_some_and(|v| v != def.vendor_id) {
            continue;
        }
        let dest = cache_path(cache_dir, def.vendor_id, def.product_id);
        // One file per board: a repo may list several layouts (ANSI/ISO) under one ID
        if dest.exists() {
            done.cached += 1;
            continue;
        }
        write(&dest, &json)?;
        done.saved += 1;
    }
    Ok(done)
}

/// Copy a VIA layout file into `user_dir` (where it wins over downloaded ones) after
/// checking it parses. Returns the board it describes.
pub fn import(file: &Path, user_dir: &Path) -> Result<Definition, LibraryError> {
    let json = std::fs::read_to_string(file).map_err(|source| LibraryError::Write {
        path: file.display().to_string(),
        source,
    })?;
    let def = Definition::parse(&json)?;
    write(&cache_path(user_dir, def.vendor_id, def.product_id), &json)?;
    Ok(def)
}

pub fn cache_path(dir: &Path, vendor_id: u16, product_id: u16) -> PathBuf {
    dir.join(format!("{vendor_id:04x}_{product_id:04x}.json"))
}

fn write(path: &Path, json: &str) -> Result<(), LibraryError> {
    let err = |source| LibraryError::Write { path: path.display().to_string(), source };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::fs::write(path, json).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: &str = r#"{
        "name": "Test", "vendorId": "0x3434", "productId": "0x0001",
        "matrix": {"rows": 1, "cols": 2},
        "layouts": {"keymap": [["0,0", "0,1"]]}
    }"#;

    fn tarball(files: &[(&str, &str)]) -> Vec<u8> {
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (path, body) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, path, body.as_bytes()).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn saves_v3_definitions_by_id_and_filters_vendor() {
        let dir = std::env::temp_dir().join(format!("delta-library-test-{}", std::process::id()));
        let other = BOARD.replace("0x3434", "0x1111");
        let tgz = tarball(&[
            ("keyboards-master/v3/test/a.json", BOARD),
            ("keyboards-master/v3/test/a_iso.json", BOARD),
            ("keyboards-master/v3/other/b.json", &other),
            ("keyboards-master/src/test/a.json", &other.replace("0x0001", "0x0002")),
            ("keyboards-master/v3/broken.json", "{}"),
        ]);
        let done = save_from_tarball(tgz.as_slice(), &dir, Some(0x3434)).unwrap();
        assert_eq!((done.saved, done.cached, done.unreadable), (1, 1, 1));
        assert!(cache_path(&dir, 0x3434, 1).exists());
        assert!(!cache_path(&dir, 0x1111, 1).exists());

        let done = save_from_tarball(tgz.as_slice(), &dir, None).unwrap();
        assert_eq!((done.saved, done.cached), (1, 2));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
