// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Manifest repositories.

use crate::ManifestStore;
use anyhow::{anyhow, Context, Error};
use fehler::{throw, throws};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// A Git repository of manifests.
#[derive(Debug)]
pub struct ManifestRepo {
    remote: String,
    working_copy: PathBuf,
}

impl ManifestRepo {
    /// Create a manifest repo cloned from the given remote.
    ///
    /// If `target_directory` exists check that it is a Git repository and has a
    #[throws]
    pub fn cloned(remote: String, target_directory: PathBuf) -> ManifestRepo {
        if target_directory.is_dir() {
            let status = Command::new("git")
                .arg("-C")
                .arg(&target_directory)
                .args(&["rev-parse", "--git-dir"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .and_then(|mut child| child.wait())
                .with_context(|| format!("Failed to run git in {}", target_directory.display()))?;
            if !status.success() {
                throw!(anyhow!(
                    "Directory {} not a Git repository",
                    target_directory.display()
                ));
            }
        } else {
            let output = Command::new("git")
                .arg("init")
                .arg(&target_directory)
                .output()
                .with_context(|| {
                    format!("Failed to run git init {}", target_directory.display())
                })?;
            if !output.status.success() {
                throw!(anyhow!(
                    "Failed to create git repository in {}: {}",
                    target_directory.display(),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        let remote_exists = Command::new("git")
            .arg("-C")
            .arg(&target_directory)
            .args(&["remote", "get-url", "homebins"])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .and_then(|mut c| c.wait())
            .map(|s| s.success())
            .unwrap_or(false);
        if !remote_exists
            && !Command::new("git")
                .arg("-C")
                .arg(&target_directory)
                .args(&["remote", "add", "homebins"])
                .arg(&remote)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .and_then(|mut c| c.wait())
                .with_context(|| format!("Failed to run git remote add origin {}", remote))?
                .success()
        {
            throw!(anyhow!("git remote add origin {} failed", remote));
        };

        let output = Command::new("git")
            .arg("-C")
            .arg(&target_directory)
            .args(&["remote", "set-url", "homebins"])
            .arg(&remote)
            .output()
            .with_context(|| format!("Failed to run git remote set-url homebins {}", remote))?;
        if !output.status.success() {
            throw!(anyhow!(
                "git remote set-url homebins {} failed: {}",
                remote,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        if !Command::new("git")
            .arg("-C")
            .arg(&target_directory)
            .args(&["fetch", "--quiet", "homebins", "master"])
            .spawn()
            .and_then(|mut c| c.wait())
            .with_context(|| {
                format!(
                    "Failed to run git fetch homebins in {}",
                    target_directory.display()
                )
            })?
            .success()
        {
            throw!(anyhow!("git fetch homebins failed"));
        }

        if !Command::new("git")
            .arg("-C")
            .arg(&target_directory)
            .args(&["reset", "--quiet", "--hard", "homebins/master"])
            .spawn()
            .and_then(|mut c| c.wait())
            .with_context(|| format!("Failed to run git reset --hard homebins/master"))?
            .success()
        {
            throw!(anyhow!("git reset --hard homebins/master failed"));
        }

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
