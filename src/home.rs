// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::ffi::OsStr;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use colored::*;
use fehler::{throw, throws};
use url::Url;
use versions::Versioning;

use anyhow::{anyhow, Context, Error, Result};

use crate::{Checksums, InstallFile, Manifest, Shell, Target};

pub struct Home {
    home: PathBuf,
    cache_dir: PathBuf,
}

#[throws]
fn curl(url: &Url, target: &Path) -> () {
    println!("curl -O {} {}", target.display(), url);
    let mut child = Command::new("curl")
        .arg("-gqb")
        .arg("")
        .arg("-fLC")
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

#[throws]
fn validate(target: &Path, checksums: &Checksums) -> () {
    let mut child = Command::new("b2sum")
        .arg("-c")
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn b2sum for {}", target.display()))?;

    let mut stdin = child.stdin.take().expect("Stdin was piped?");
    write!(stdin, "{} ", checksums.b2)
        .and_then(|_| stdin.write_all(target.as_os_str().as_bytes()))
        .with_context(|| format!("Failed to pipe checksum to b2sum for {}", target.display()))?;
    drop(stdin);

    let status = child
        .wait()
        .with_context(|| format!("Failed to wait for b2sum for {}", target.display()))?;

    if !status.success() {
        throw!(anyhow!("b2sum failed to validate {}", target.display()));
    }
}

#[throws]
fn maybe_extract(directory: &Path, file: &Path) -> () {
    let filename = file.file_name().unwrap_or_default().to_string_lossy();
    if filename.ends_with(".tar.gz")
        || filename.ends_with(".tar.bz2")
        || filename.ends_with(".tar.xz")
    {
        println!("tar xf {}", file.display());
        let status = Command::new("tar")
            .arg("xf")
            .arg(file)
            .arg("-C")
            .arg(directory)
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to spawn tar xf {} -C {}",
                    file.display(),
                    directory.display()
                )
            })?
            .wait()
            .with_context(|| {
                format!(
                    "Failed to wait for spawn tar xf {} -C {}",
                    file.display(),
                    directory.display()
                )
            })?;

        if !status.success() {
            throw!(anyhow!(
                "tar xf {} -C {} failed with exit code {}",
                file.display(),
                directory.display(),
                status,
            ))
        }
    }
}

impl Home {
    pub fn new() -> Home {
        // if $HOME or ~/.cache doesn't exist we're really screwed so let's just panic
        let home = dirs::home_dir().unwrap();
        let cache_dir = dirs::cache_dir().map(|d| d.join("homebins")).unwrap();
        Home { home, cache_dir }
    }

    fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.download_dir()).with_context(|| {
            format!(
                "Failed to create download directory at {}",
                self.download_dir().display()
            )
        })
    }

    pub fn download_dir(&self) -> PathBuf {
        self.cache_dir.join("downloads")
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.home.join(".local").join("bin")
    }

    pub fn man_dir(&self, section: u8) -> PathBuf {
        self.home
            .join(".local")
            .join("share")
            .join("man")
            .join(format!("man{}", section))
    }

    #[throws]
    pub fn installed_manifest_version(&self, manifest: &Manifest) -> Option<Versioning> {
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
                    manifest.info.name, manifest.discover.version_check.pattern
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

            Some(Versioning::new(version).ok_or_else(|| {
                anyhow!(
                    "Output of command {} with {:?} returned invalid version {:?}",
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
    pub fn target(&self, file: &InstallFile) -> PathBuf {
        let name: &OsStr = match &file.name {
            Some(name) => name.as_ref(),
            None => file.source.file_name().ok_or_else(|| {
                anyhow!(
                    "name not set for file and no file name in {}",
                    file.source.display()
                )
            })?,
        };
        match file.target {
            Target::Binary => self.bin_dir().join(name),
            Target::Manpage { section } => self.man_dir(section).join(name),
            Target::Completion { shell: Shell::Fish } => dirs::config_dir()
                .unwrap()
                .join("fish")
                .join("completions")
                .join(name),
        }
    }

    #[throws]
    pub fn install_manifest(&mut self, manifest: &Manifest) -> () {
        self.ensure_dirs()?;
        let work_dir = tempfile::tempdir().with_context(|| {
            format!(
                "Failed to create temporary directory to install {}",
                manifest.info.name
            )
        })?;

        for install in &manifest.install {
            let target = self.download_dir().join(install.filename()?);
            if !target.is_file() {
                println!("Downloading {}", install.download.as_str().bold());
                curl(&install.download, &target)?;
            }
            validate(&target, &install.checksums)?;
            maybe_extract(work_dir.path(), &target)?;
            for file in &install.files {
                let source = work_dir.path().join(&file.source);
                let target = self.target(file)?;
                let mode = if file.is_executable() { 0o755 } else { 0o644 };
                println!(
                    "install -m{:o} {} {}",
                    mode,
                    file.source.display(),
                    target.display()
                );
                std::fs::create_dir_all(target.parent().expect("Target must be absolute by now"))?;
                std::fs::copy(&source, &target).with_context(|| {
                    format!(
                        "Failed to copy {} to {}",
                        &file.source.display(),
                        target.display()
                    )
                })?;
                let mut permissions = std::fs::metadata(&target)?.permissions();
                permissions.set_mode(mode);
                std::fs::set_permissions(&target, permissions).with_context(|| {
                    format!(
                        "Failed to set mode {:o} on installed file {}",
                        mode,
                        target.display()
                    )
                })?;
            }
        }
    }
}
