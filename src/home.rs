// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use colored::*;
use fehler::throws;
use versions::Versioning;

use anyhow::{anyhow, Context, Error, Result};

use crate::checksum::Validate;
use crate::tools::*;
use crate::{Install, Manifest, ManifestRepo, ManifestStore, Shell, Target};
use std::fs::File;

/// The home directory.
///
/// Keeps track of the $HOME path and other directories we need to access.
pub struct Home {
    home: PathBuf,
    cache_dir: PathBuf,
}

fn path_contains<S: AsRef<OsStr>, P: AsRef<Path>>(path: &S, wanted: P) -> bool {
    std::env::split_paths(path).any(|path| path.as_path() == wanted.as_ref())
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

    /// Check whether the environment is ok, and print warnings to stderr if not.
    #[throws]
    pub fn check_environment(&self) -> () {
        match std::env::var_os("PATH") {
            None => eprintln!("{}", "WARNING: $PATH not set!".yellow().bold()),
            Some(path) => {
                if !path_contains(&path, self.bin_dir()) {
                    eprintln!(
                        "{}\nAdd {} to $PATH in your shell profile.",
                        format!(
                            "WARNING: $PATH does not contain bin dir at {}",
                            self.bin_dir().display()
                        )
                        .yellow()
                        .bold(),
                        self.bin_dir().display()
                    )
                }
            }
        };

        if !path_contains(&manpath()?, self.man_dir()) {
            eprintln!(
                "{}\nAdd {} to $MANPATH in your shell profile; see man 1 manpath for more information",
                format!(
                    "WARNING: manpath does not contain man dir at {}",
                    self.man_dir().display()
                )
                .yellow()
                .bold(),
                self.man_dir().display()
            );
        }
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

    /// Get the man directory.
    ///
    /// This is `$HOME/.local/share/man`.
    pub fn man_dir(&self) -> PathBuf {
        self.home.join(".local").join("share").join("man")
    }

    /// The directory to install man pages of the given section to.
    ///
    /// This is the corresponding sub-directory of the man_dir.
    pub fn man_section_dir(&self, section: u8) -> PathBuf {
        self.man_dir().join(format!("man{}", section))
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

    /// Whether the given manifest is outdated and needs updating.
    ///
    /// Return the installed version if it's outdated, otherwise return None.
    #[throws]
    pub fn outdated_manifest_version(&self, manifest: &Manifest) -> Option<Versioning> {
        self.installed_manifest_version(manifest)?
            .filter(|installed| installed < &manifest.info.version)
    }

    /// The target directory for a given target.
    pub fn target_dir(&self, target: Target) -> PathBuf {
        match target {
            Target::Binary => self.bin_dir(),
            Target::Manpage { section } => self.man_section_dir(section),
            Target::Completion { shell: Shell::Fish } => {
                dirs::config_dir().unwrap().join("fish").join("completions")
            }
        }
    }

    /// Obtain the name of a target file from the given explicit name or the source.
    pub fn target_name<'a>(&self, source: &'a Path, name: Option<&'a str>) -> Result<&'a OsStr> {
        match name {
            Some(name) => Ok(name.as_ref()),
            None => source.file_name().ok_or_else(|| {
                anyhow!(
                    "name not set for file and no file name in {}",
                    source.display()
                )
            }),
        }
    }

    /// Get all files a given manifest would install.
    #[throws]
    pub fn installed_files(&self, manifest: &Manifest) -> Vec<PathBuf> {
        let mut installed_files = Vec::with_capacity(&manifest.install.len() * 3);
        for install in &manifest.install {
            match &install.install {
                Install::SingleFile { name, target } => installed_files.push(
                    self.target_dir(*target)
                        .join(self.target_name(&Path::new(install.filename()?), name.as_deref())?),
                ),
                Install::FilesFromArchive { files } => {
                    for file in files {
                        installed_files.push(
                            self.target_dir(file.target)
                                .join(self.target_name(&file.source, file.name.as_deref())?),
                        )
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

        let target_dir = self.target_dir(target);
        let target_name = self.target_name(source.as_ref(), name)?;
        let target = target_dir.join(target_name);
        println!(
            "install -m{:o} {} {}",
            mode,
            source.as_ref().display(),
            target.display()
        );
        std::fs::create_dir_all(&target_dir)?;
        // Copy file to temporary file along the target, then rename, replacing the original
        let mut temp_target = tempfile::Builder::new()
            .prefix(target_name)
            .tempfile_in(&target_dir)
            .with_context(|| {
                format!(
                    "Failed to create temporary target file in {}",
                    target_dir.display()
                )
            })?;
        std::io::copy(&mut File::open(&source)?, &mut temp_target).with_context(|| {
            format!(
                "Failed to copy {} to {}",
                source.as_ref().display(),
                temp_target.path().display()
            )
        })?;
        temp_target
            .persist(&target)
            .with_context(|| format!("Failed to persist at {}", target.display()))?;
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
            install
                .checksums
                .validate(&mut File::open(&download).with_context(|| {
                    format!(
                        "Failed to open {} for checksum validation",
                        download.display(),
                    )
                })?)
                .with_context(|| format!("Failed to validate {}", download.display()))?;
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

    /// Remove all files installed by the given manifest.
    ///
    /// Returns all removed files.
    #[throws]
    pub fn remove_manifest(&mut self, manifest: &Manifest) -> Vec<PathBuf> {
        let installed_files = self.installed_files(manifest)?;
        let mut removed_files = Vec::with_capacity(installed_files.len());
        for file in installed_files {
            if file.exists() {
                std::fs::remove_file(&file).with_context(|| {
                    format!(
                        "Failed to remove {} while removing {}",
                        file.display(),
                        &manifest.info.name,
                    )
                })?;
                removed_files.push(file);
            }
        }
        removed_files
    }
}
