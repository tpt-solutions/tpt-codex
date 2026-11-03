// SPDX-License-Identifier: MIT OR Apache-2.0
//! A [`Storage`] backend on the local file system.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use tpt_codex_core::Storage;

/// Stores each key as a file under `root`.
///
/// Keys must be relative paths made of normal components (no `..`, no absolute paths,
/// no drive prefixes). Writes go to a temporary file first and are then renamed into
/// place, so a crash never leaves a half-written document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStorage {
    root: PathBuf,
}

impl FileStorage {
    /// Uses `root` as the storage directory. It is created on first write.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The storage directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, key: &str) -> io::Result<PathBuf> {
        let rel = Path::new(key);
        let mut out = self.root.clone();
        let mut any = false;
        for component in rel.components() {
            match component {
                Component::Normal(part) => {
                    out.push(part);
                    any = true;
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unsafe storage key '{key}'"),
                    ));
                }
            }
        }
        if !any {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty storage key",
            ));
        }
        Ok(out)
    }
}

impl Storage for FileStorage {
    type Error = io::Error;

    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, io::Error> {
        match fs::read(self.path_for(key)?) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn write(&mut self, key: &str, bytes: &[u8]) -> Result<(), io::Error> {
        let path = self.path_for(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut tmp = path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &path)
    }

    fn remove(&mut self, key: &str) -> Result<(), io::Error> {
        match fs::remove_file(self.path_for(key)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}
