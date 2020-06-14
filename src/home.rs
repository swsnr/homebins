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
use versions::Versioning;

use anyhow::{anyhow, Context, Error, Result};

use crate::tools::*;
use crate::{Checksums, Install, Manifest, ManifestRepo, ManifestStore, Shell, Target};

/// The home directory.
///
/// Keeps track of the $HOME path and other directories we need to access.
pub struct Home {
    home: PathBuf,
    cache_dir: PathBuf,
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

impl Home {
    /// Open the real $HOME directory.
    ///
    /// This function can panic if the dirs crate cannot figure out either $HOME or $XDG_CACHE_DIR.
    pub fn open() -> Home {
        // if $HOME or ~/.cache doesn't exist we're really screwed so let's just panic
        let home = dirs::home_dir().unwrap();
        let cache_dir = dirs::cache_dir().map(|d| d.join("homebins")).unwrap();
        Home { home, cache_dir }
    }

    /// The directory to download files from manifests to.
    ///
    /// This is a subdirectory of our cache directory.
    pub fn download_dir(&self) -> PathBuf {
        self.cache_dir.join("downloads")
    }

    /// The download directory for a specific manifest.
    ///
    /// This is a subdirectory of the download directory with the name and
    /// the version of the given manifest.
    pub fn manifest_download_dir(&self, manifest: &Manifest) -> PathBuf {
        self.download_dir()
            .join(&manifest.info.name)
            .join(&manifest.info.version.to_string())
    }

    /// The directory to clone manifest repositories to.
    ///
    /// This is a subdirectory of our cache directory.
    pub fn manifest_repos_dir(&self) -> PathBuf {
        self.cache_dir.join("manifest_repos")
    }

    /// The directory to install binaries to.
    ///
    /// This is `$HOME/.local/bin`.
    pub fn bin_dir(&self) -> PathBuf {
        self.home.join(".local").join("bin")
    }

    /// The directory to install man pages of the given section to.
    ///
    /// This is the corresponding sub-directory of `$HOME/.local/share/man`.
    pub fn man_dir(&self, section: u8) -> PathBuf {
        self.home
            .join(".local")
            .join("share")
            .join("man")
            .join(format!("man{}", section))
    }

    /// Clone a manifest repository from the given remote under the given name.
    ///
    /// The repository gets cloned to a subdirectory of [`manifest_repos_dir`].
    /// See [`ManifestRepo::cloned`] for details.
    fn cloned_manifest_repo(&mut self, remote: String, name: &str) -> Result<ManifestRepo> {
        let dir = self.manifest_repos_dir();
        std::fs::create_dir_all(&dir).with_context(|| {
            format!(
                "Failed to create directory for manifest repos at {}",
                dir.display()
            )
        })?;
        ManifestRepo::cloned(remote, dir.join(name))
    }

    /// Get the manifest store to install from.
    pub fn manifest_store(&mut self) -> Result<ManifestStore> {
        self.cloned_manifest_repo(
            "https://github.com/lunaryorn/homebin-manifests".into(),
            "lunaryorn",
        )
        .map(|repo| repo.store())
    }

    /// Get the installed version of the given manifest.
    ///
    /// Attempt to invoke the version check denoted in the manifest, i.e. the given binary with the
    /// version check arguments, and use the pattern to extract a version number.
    ///
    /// Return `None` if the binary doesn't exist; fail if we cannot invoke it for other reasons or
    /// if we fail to parse the version from other.
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

    /// Get the file system target for the given file to install.
    ///
    /// Return the path which we must copy the given file to.  Fails if the file has no explicit
    /// file name and no file name in its source.
    #[throws]
    pub fn target<P: AsRef<Path>>(&self, source: P, name: Option<&str>, target: Target) -> PathBuf {
        let name: &OsStr = match name {
            Some(name) => name.as_ref(),
            None => source.as_ref().file_name().ok_or_else(|| {
                anyhow!(
                    "name not set for file and no file name in {}",
                    source.as_ref().display()
                )
            })?,
        };
        match target {
            Target::Binary => self.bin_dir().join(name),
            Target::Manpage { section } => self.man_dir(section).join(name),
            Target::Completion { shell: Shell::Fish } => dirs::config_dir()
                .unwrap()
                .join("fish")
                .join("completions")
                .join(name),
        }
    }

    /// Get all files a given manifest would install.
    #[throws]
    pub fn installed_files(&self, manifest: &Manifest) -> Vec<PathBuf> {
        let mut installed_files = Vec::with_capacity(&manifest.install.len() * 3);
        for install in &manifest.install {
            match &install.install {
                Install::SingleFile { name, target } => installed_files.push(self.target(
                    install.filename()?,
                    name.as_deref(),
                    *target,
                )?),
                Install::FilesFromArchive { files } => {
                    for file in files {
                        installed_files.push(self.target(
                            &file.source,
                            file.name.as_deref(),
                            file.target,
                        )?)
                    }
                }
            }
        }
        installed_files
    }

    /// Install the single given file.
    #[throws]
    fn install_file<P: AsRef<Path>>(
        &mut self,
        source: P,
        name: Option<&str>,
        target: Target,
    ) -> () {
        let mode = if target.is_executable() { 0o755 } else { 0o644 };
        let target = self.target(&source, name, target)?;
        println!(
            "install -m{:o} {} {}",
            mode,
            source.as_ref().display(),
            target.display()
        );
        std::fs::create_dir_all(target.parent().expect("Target must be absolute by now"))?;
        std::fs::copy(&source, &target).with_context(|| {
            format!(
                "Failed to copy {} to {}",
                source.as_ref().display(),
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

    /// Install a manifest.
    ///
    /// Download the files denoted by the given manifest, extract as needed and then copy files to
    /// $HOME as described in the manifest.
    #[throws]
    pub fn install_manifest(&mut self, manifest: &Manifest) -> () {
        let download_directory = self.manifest_download_dir(manifest);
        std::fs::create_dir_all(&download_directory).with_context(|| {
            format!(
                "Failed to create download directory at {}",
                self.download_dir().display()
            )
        })?;

        let work_dir = tempfile::tempdir().with_context(|| {
            format!(
                "Failed to create temporary directory to install {}",
                manifest.info.name
            )
        })?;

        for install in &manifest.install {
            let download = download_directory.join(install.filename()?);
            if !download.is_file() {
                println!("Downloading {}", install.download.as_str().bold());
                curl(&install.download, &download)?;
            }
            validate(&download, &install.checksums)?;
            match &install.install {
                Install::FilesFromArchive { files } => {
                    maybe_extract(&download, work_dir.path())?;
                    for file in files {
                        self.install_file(
                            work_dir.path().join(&file.source),
                            file.name.as_deref(),
                            file.target,
                        )?;
                    }
                }
                Install::SingleFile { name, target } => {
                    self.install_file(&download, name.as_deref(), *target)?;
                }
            };
        }
    }
}
