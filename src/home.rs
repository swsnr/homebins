// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::PathBuf;
use std::process::Command;

use fehler::throws;
use semver::Version;

use anyhow::{anyhow, Context, Error, Result};

use crate::Manifest;

pub struct Home {
    home: PathBuf,
}

impl Home {
    pub fn new() -> Result<Home> {
        dirs::home_dir()
            .ok_or(anyhow!("Home directory does not exist"))
            .map(|home| Home { home })
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.home.join(".local").join("bin")
    }

    #[throws]
    pub fn installed_manifest_version(&self, manifest: &Manifest) -> Option<Version> {
        let args = &manifest.discover.version_check.args;
        let binary = self.bin_dir().join(&manifest.discover.binary);
        if binary.is_file() {
            let output = Command::new(&binary).args(args).output().with_context(|| {
                format!(
                    "Failed to run {} with {:?}",
                    binary.display(),
                    &manifest.discover.version_check.args
                )
            })?;
            let pattern = manifest.discover.version_check.regex().with_context(|| {
                format!(
                    "Version check for {} failed: Invalid regex {}",
                    manifest.meta.name, manifest.discover.version_check.pattern
                )
            })?;
            let output = std::str::from_utf8(&output.stdout).with_context(|| {
                format!(
                    "Output of command {} with {:?} returned non-utf8 stdout: {:?}",
                    binary.display(),
                    args,
                    output.stdout
                )
            })?;
            let version = pattern
                .captures(output)
                .ok_or(anyhow!(
                    "Output of command {} with {:?} did not contain match for {}: {}",
                    binary.display(),
                    args,
                    manifest.discover.version_check.pattern,
                    output
                ))?
                .get(1)
                .ok_or(anyhow!(
                    "Output of command {} with {:?} did not contain capture group 1 for {}: {}",
                    binary.display(),
                    args,
                    manifest.discover.version_check.pattern,
                    output
                ))?
                .as_str();
            Some(Version::parse(version).with_context(|| format!("Invalid version"))?)
        } else {
            None
        }
    }
}
