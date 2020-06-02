// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Error, Result};
use colored::*;
use fehler::{throw, throws};
use semver::Version;

use crate::Manifest;
use url::Url;

pub struct Home {
    home: PathBuf,
}

#[throws]
fn curl(url: &Url, target: &Path) -> () {
    let mut child = Command::new("curl")
        .arg("--disable")
        .arg("--globoff")
        .arg("--cookie")
        .arg("")
        .arg("--fail")
        .arg("--location")
        .arg("--continue-at")
        .arg("-")
        .arg("--progress-bar")
        .arg("--retry")
        .arg("3")
        .arg("--retry-delay")
        .arg("3")
        .arg("--output")
        .arg(target)
        .arg(url.as_str())
        .spawn()
        .with_context(|| {
            format!(
                "Failed start curl to download {} to {}",
                url,
                target.display()
            )
        })?;
    let status = child.wait().with_context(|| {
        format!(
            "Failed to wait for curl to download {} to {}",
            url,
            target.display()
        )
    })?;
    if !status.success() {
        throw!(anyhow!(
            "Failed to download {} to {}",
            url,
            target.display(),
        ))
    }
}

impl Home {
    pub fn new() -> Result<Home> {
        dirs::home_dir()
            .ok_or_else(|| anyhow!("Home directory does not exist"))
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
                .ok_or_else(|| {
                    anyhow!(
                        "Output of command {} with {:?} did not contain match for {}: {}",
                        binary.display(),
                        args,
                        manifest.discover.version_check.pattern,
                        output
                    )
                })?
                .get(1)
                .ok_or_else(|| {
                    anyhow!(
                        "Output of command {} with {:?} did not contain capture group 1 for {}: {}",
                        binary.display(),
                        args,
                        manifest.discover.version_check.pattern,
                        output
                    )
                })?
                .as_str();
            Some(Version::parse(version).with_context(|| {
                format!(
                    "Output of command {} with {:?} returned invalid version {}",
                    binary.display(),
                    args,
                    version
                )
            })?)
        } else {
            None
        }
    }

    #[throws]
    pub fn install_manifest(&mut self, manifest: &Manifest) -> () {
        let work_dir = tempfile::tempdir().with_context(|| {
            format!(
                "Failed to create temporary directory to install {}",
                manifest.meta.name
            )
        })?;

        for install in &manifest.install {
            println!("Downloading {}", install.download.as_str().bold());
            curl(
                &install.download,
                &work_dir.path().join(install.filename()?),
            )?;
        }
        Command::new("ls")
            .arg("-la")
            .current_dir(work_dir.path())
            .spawn()?
            .wait()?;
    }
}
