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

use crate::{Checksums, InstallFile, Manifest, ManifestRepo, ManifestStore, Shell, Target};

/// The home directory.
///
/// Keeps track of the $HOME path and other directories we need to access.
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
fn untar(archive: &Path, target_directory: &Path) -> () {
    println!("tar xf {}", archive.display());
    let status = Command::new("tar")
        .arg("xf")
        .arg(archive)
        .arg("-C")
        .arg(target_directory)
        .spawn()
        .and_then(|mut c| c.wait())
        .with_context(|| {
            format!(
                "Failed to spawn tar xf {} -C {}",
                archive.display(),
                target_directory.display()
            )
        })?;

    if !status.success() {
        throw!(anyhow!(
            "tar xf {} -C {} failed with exit code {}",
            archive.display(),
            target_directory.display(),
            status,
        ))
    }
}

#[throws]
fn unzip(archive: &Path, target_directory: &Path) -> () {
    println!("unzip {}", archive.display());
    let status = Command::new("unzip")
        .arg(archive)
        .arg("-d")
        .arg(target_directory)
        .spawn()
        .and_then(|mut c| c.wait())
        .with_context(|| {
            format!(
                "Failed to spawn unzip {} -d {}",
                archive.display(),
                target_directory.display()
            )
        })?;

    if !status.success() {
        throw!(anyhow!(
            "unzip {} -d {} failed with exit code {}",
            archive.display(),
            target_directory.display(),
            status,
        ))
    }
}

// There's no point in making a type alias for this one single type.
#[allow(clippy::type_complexity)]
static ARCHIVE_PATTERNS: [(&str, fn(&Path, &Path) -> Result<()>); 5] = [
    (".tar.gz", untar),
    (".tgz", untar),
    (".tar.bz2", untar),
    (".tar.xz", untar),
    ("zip", unzip),
];

#[throws]
fn maybe_extract(file: &Path, directory: &Path) -> () {
    for (extension, extract) in &ARCHIVE_PATTERNS {
        if file.as_os_str().to_string_lossy().ends_with(extension) {
            extract(file, directory)?;
            return ();
        }
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
            let target = download_directory.join(install.filename()?);
            if !target.is_file() {
                println!("Downloading {}", install.download.as_str().bold());
                curl(&install.download, &target)?;
            }
            validate(&target, &install.checksums)?;
            maybe_extract(&target, work_dir.path())?;
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
