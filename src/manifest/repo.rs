// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Manifest repositories.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Error};
use fehler::throws;

use crate::process::CommandExt;
use crate::tools::git;
use crate::ManifestStore;

/// A Git repository of manifests.
#[derive(Debug)]
pub struct ManifestRepo {
    remote: String,
    working_copy: PathBuf,
}

#[throws]
fn clone_repo(remote: &str, target_directory: &Path) -> () {
    if target_directory.is_dir() {
        git(target_directory)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .args(&["rev-parse", "--git-dir"])
            .checked_call()
            .with_context(|| {
                format!(
                    "Directory {} not a Git repository",
                    target_directory.display()
                )
            })?;
    } else {
        Command::new("git")
            .arg("init")
            .arg(target_directory)
            .checked_output()
            .with_context(|| {
                format!(
                    "Failed to create git repository in {}",
                    target_directory.display(),
                )
            })?;
    }

    let remote_exists = git(&target_directory)
        .args(&["remote", "get-url", "homebins"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .call()
        .map(|s| s.success())
        .unwrap_or(false);
    if !remote_exists {
        git(&target_directory)
            .args(&["remote", "add", "homebins"])
            .arg(&remote)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .checked_call()?;
    }

    git(target_directory)
        .args(&["remote", "set-url", "homebins"])
        .arg(&remote)
        .checked_call()?;

    git(target_directory)
        .args(&["fetch", "--quiet", "homebins", "master"])
        .checked_call()?;

    git(target_directory)
        .args(&["reset", "--quiet", "--hard", "homebins/master"])
        .checked_call()?;
}

impl ManifestRepo {
    /// Create a manifest repo cloned from the given remote.
    ///
    /// If `target_directory` exists check that it is a Git repository and has a
    #[throws]
    pub fn cloned(remote: String, target_directory: PathBuf) -> ManifestRepo {
        clone_repo(&remote, &target_directory).with_context(|| {
            format!(
                "Failed to clone {} to {}",
                remote,
                target_directory.display()
            )
        })?;
        ManifestRepo {
            remote,
            working_copy: target_directory,
        }
    }

    /// Get the store this repository has cloned.
    ///
    /// The store must be in the `manifests/` subdirectory of the repository.
    pub fn store(&self) -> ManifestStore {
        ManifestStore::open(self.working_copy.join("manifests"))
    }
}
