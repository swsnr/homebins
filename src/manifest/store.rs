// Copyright Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Error, Result};
use fehler::throws;

use super::types::Manifest;

/// A store of manifests.
#[derive(Debug)]
pub struct ManifestStore {
    base_dir: PathBuf,
}

impl ManifestStore {
    /// Open a directory of manifests.
    ///
    /// Does not fail because this method doesn't attempt to access `base_dir` just yet.
    pub fn open(base_dir: PathBuf) -> ManifestStore {
        ManifestStore { base_dir }
    }

    /// Load a manifest from this store.
    ///
    /// Return the manifest if it exists or None if the store has no manifest with the given name.
    /// Fail if the store doesn't exist or isn't readable.
    pub fn load_manifest<S: AsRef<str>>(&self, name: S) -> Result<Option<Manifest>> {
        let manifest_file = self.base_dir.join(name.as_ref()).with_extension("toml");
        if name.as_ref().is_empty()
            || manifest_file.file_stem().unwrap_or_default() != name.as_ref()
        {
            // If the stem of the manifest isn't the name we got a name with a path separator
            // which we definitely don't accept.
            Err(anyhow!("Invalid manifest name: {}", name.as_ref()))
        } else {
            Manifest::read_from_path(manifest_file)
                .map(Some)
                .or_else(|error| match error.downcast_ref::<std::io::Error>() {
                    Some(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    _ => Err(error),
                })
        }
    }

    /// Iterate over all manifests in this store.
    #[throws]
    pub fn manifests(&self) -> impl Iterator<Item = Result<Manifest>> {
        self.base_dir
            .read_dir()
            .with_context(|| {
                format!(
                    "Failed to open manifest store at {}",
                    self.base_dir.display()
                )
            })?
            .map(|item| match item {
                Ok(entry) => Manifest::read_from_path(entry.path()),
                Err(err) => Err(Error::new(err)),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn load_existing_manifest() {
        let store = ManifestStore::open(Path::new("tests/manifests/").to_path_buf());
        let manifest = store.load_manifest("ripgrep").unwrap().unwrap();
        assert_eq!(manifest.info.name, "ripgrep");
    }

    #[test]
    fn load_empty_name() {
        let store = ManifestStore::open(Path::new("manifests/").to_path_buf());
        assert!(store
            .load_manifest("")
            .unwrap_err()
            .to_string()
            .contains("Invalid manifest name"))
    }

    #[test]
    fn load_invalid_name() {
        let store = ManifestStore::open(Path::new("manifests/").to_path_buf());
        assert!(store
            .load_manifest("foo/bar")
            .unwrap_err()
            .to_string()
            .contains("Invalid manifest name"))
    }

    #[test]
    fn load_non_existing_manifest() {
        let store = ManifestStore::open(Path::new("manifests/").to_path_buf());
        assert!(store.load_manifest("non-existing").unwrap().is_none())
    }
}
