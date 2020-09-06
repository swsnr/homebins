// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::{HomebinProjectDirs, ManifestRepo, ManifestStore};
use anyhow::{Context, Result};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// All homebin repos
#[derive(Debug)]
pub struct HomebinRepos<'a> {
    repos_dir: Cow<'a, Path>,
}

impl<'a> HomebinRepos<'a> {
    /// Load homebin manifest repositorie from the given path.
    pub fn new(repos_dir: PathBuf) -> HomebinRepos<'a> {
        HomebinRepos {
            repos_dir: Cow::Owned(repos_dir),
        }
    }

    /// Load homebie manifest repositories from homebin project dirs.
    ///
    /// The manifest repos are at CACHE_DIR/manifeset_repos.
    pub fn open(dirs: &HomebinProjectDirs) -> HomebinRepos {
        HomebinRepos {
            repos_dir: Cow::Borrowed(dirs.repos_dir()),
        }
    }

    /// Clone a manifest repository from the given remote under the given name.
    ///
    /// The repository gets cloned to a subdirectory of the manifest repos dir.
    /// See [`ManifestRepo::cloned`] for details.
    fn cloned_manifest_repo(&mut self, remote: String, name: &str) -> Result<ManifestRepo> {
        std::fs::create_dir_all(&self.repos_dir).with_context(|| {
            format!(
                "Failed to create directory for manifest repos at {}",
                self.repos_dir.display()
            )
        })?;
        ManifestRepo::cloned(remote, self.repos_dir.join(name))
    }

    /// Get the manifest store to install from.
    ///
    /// This store aggregates all manifest repos.
    pub fn manifest_store(&mut self) -> Result<ManifestStore> {
        self.cloned_manifest_repo(
            "https://github.com/lunaryorn/homebin-manifests".into(),
            "lunaryorn",
        )
        .map(|repo| repo.store())
    }
}
