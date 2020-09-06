// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Install binaries to $HOME.
//!
//! Not a package manager.

#![deny(warnings, clippy::all, missing_docs)]

use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use anyhow::{anyhow, Context, Error};
use colored::Colorize;
use fehler::throws;
use versions::Versioning;

pub use dirs::*;
pub use manifest::{Manifest, ManifestRepo, ManifestStore};
pub use repos::HomebinRepos;

use crate::checksum::Validate;
use crate::operations::{Operation, Source};
use crate::tools::{curl, extract, manpath, path_contains};
use std::path::PathBuf;

mod checksum;
mod dirs;
mod process;
mod repos;
mod tools;

/// Manifest types and loading.
pub mod manifest;
/// Operations to apply manifests to a home directory.
pub mod operations;

/// Check whether the environment is ok, and print warnings to stderr if not.
///
/// This specifically checks whether `install_dirs` are contained in the relevant environment variables
/// such as `$PATH` or `$MANPATH`.
#[throws]
pub fn check_environment(install_dirs: &InstallDirs) -> () {
    match std::env::var_os("PATH") {
        None => eprintln!("{}", "WARNING: $PATH not set!".yellow().bold()),
        Some(path) => {
            if !path_contains(&path, install_dirs.bin_dir()) {
                eprintln!(
                    "{}\nAdd {} to $PATH in your shell profile.",
                    format!(
                        "WARNING: $PATH does not contain bin dir at {}",
                        install_dirs.bin_dir().display()
                    )
                    .yellow()
                    .bold(),
                    install_dirs.bin_dir().display()
                )
            }
        }
    };

    if !path_contains(&manpath()?, install_dirs.man_dir()) {
        eprintln!(
            "{}\nAdd {} to $MANPATH in your shell profile; see man 1 manpath for more information",
            format!(
                "WARNING: manpath does not contain man dir at {}",
                install_dirs.man_dir().display()
            )
            .yellow()
            .bold(),
            install_dirs.man_dir().display()
        );
    }
}

/// Apply operations to directories.
#[throws]
pub fn apply_operations<'a, I>(dirs: ManifestOperationDirs, operations: I) -> ()
where
    I: Iterator<Item = &'a Operation<'a>>,
{
    std::fs::create_dir_all(dirs.download_dir()).with_context(|| {
        format!(
            "Failed to create download directory at {}",
            dirs.download_dir().display()
        )
    })?;

    use Operation::*;

    for operation in operations {
        match operation {
            Download(url, name, checksums) => {
                println!("Downloading {}", url.as_str().bold());
                let dest = dirs.download_dir().join(name.as_ref());
                // FIXME: Don't check for file, instead handle 416 errors from curl as indicator for completeness
                if !dest.exists() {
                    curl(&url, &dest)?;
                }
                let mut source = &mut File::open(&dest).with_context(|| {
                    format!("Failed to open {} for checksum validation", dest.display())
                })?;
                checksums
                    .validate(&mut source)
                    .with_context(|| format!("Failed to validate {}", dest.display()))?;
            }
            Extract(name) => {
                extract(&dirs.download_dir().join(name.as_ref()), dirs.work_dir())?;
            }
            Copy(source, destination, permissions) => {
                let fs_permissions = permissions.to_unix_permissions();
                let mode = fs_permissions.mode();
                let (source_name, source_path) = match source {
                    Source::Download(name) => (name, dirs.download_dir().join(name.as_ref())),
                    Source::WorkDir(name) => (name, dirs.work_dir().join(name.as_ref())),
                };
                let target_dir = destination.target_dir(dirs.install_dirs());
                let target_name = destination.target_name();
                let target = target_dir.join(target_name);
                println!("install -m{:o} {} {}", mode, source_name, target.display());
                std::fs::create_dir_all(&target_dir)?;
                let mut temp_target = tempfile::Builder::new()
                    .prefix(target_name)
                    .tempfile_in(&target_dir)
                    .with_context(|| {
                        format!(
                            "Failed to create temporary target file in {}",
                            target_dir.display()
                        )
                    })?;
                std::io::copy(&mut File::open(&source_path)?, &mut temp_target).with_context(
                    || {
                        format!(
                            "Failed to copy {} to {}",
                            source_path.display(),
                            temp_target.path().display()
                        )
                    },
                )?;
                temp_target
                    .persist(&target)
                    .with_context(|| format!("Failed to persist at {}", target.display()))?;
                std::fs::set_permissions(&target, fs_permissions).with_context(|| {
                    format!(
                        "Failed to set mode {:o} on installed file {}",
                        mode,
                        target.display()
                    )
                })?;
            }
            Hardlink(source, target) => {
                let src = dirs.install_dirs().bin_dir().join(source.as_ref());
                let dst = dirs.install_dirs().bin_dir().join(target.as_ref());
                println!("ln -f {} {}", src.display(), dst.display());
                if dst.exists() {
                    std::fs::remove_file(&dst)
                        .with_context(|| format!("Failed to override {}", dst.display()))?;
                }
                std::fs::hard_link(&src, &dst).with_context(|| {
                    format!("Failed to link {} to {}", src.display(), dst.display(),)
                })?;
            }
        }
    }
}

/// Install a manifest.
///
/// Apply the operations of a `manifest` against the given `install_dirs`; using the given project `dirs` for downloads.
#[throws]
pub fn install_manifest(
    dirs: &HomebinProjectDirs,
    install_dirs: &mut InstallDirs,
    manifest: &Manifest,
) -> () {
    let operations = operations::install_manifest(manifest);
    let op_dirs = ManifestOperationDirs::for_manifest(dirs, install_dirs, manifest)?;
    apply_operations(op_dirs, operations.iter())?;
}

/// Get the installed version of the given manifest.
///
/// Attempt to invoke the version check denoted in the manifest, i.e. the given binary with the
/// version check arguments, and use the pattern to extract a version number.
///
/// Return `None` if the binary doesn't exist or its output doesn't match the pattern;
/// fail if we cannot invoke it for other reasons or if we fail to parse the version from other.
#[throws]
pub fn installed_manifest_version(dirs: &InstallDirs, manifest: &Manifest) -> Option<Versioning> {
    let args = &manifest.discover.version_check.args;
    let binary = dirs.bin_dir().join(&manifest.discover.binary);
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
            .and_then(|c| c.get(1))
            .map(|m| m.as_str());

        version
            .map(|s| {
                Versioning::new(s).ok_or_else(|| {
                    anyhow!(
                        "Output of command {} with {:?} returned invalid version {:?}",
                        binary.display(),
                        args,
                        version
                    )
                })
            })
            .transpose()?
    } else {
        None
    }
}

/// Whether the given manifest is outdated and needs updating.
///
/// Return the installed version if it's outdated, otherwise return None.
#[throws]
pub fn outdated_manifest_version(dirs: &InstallDirs, manifest: &Manifest) -> Option<Versioning> {
    installed_manifest_version(dirs, manifest)?
        .filter(|installed| installed < &manifest.info.version)
}

/// Get all files the `manifest` would install to `dirs`.
pub fn files(dirs: &InstallDirs, manifest: &Manifest) -> Vec<PathBuf> {
    operations::operation_destinations(operations::install_manifest(manifest).iter())
        .iter()
        .map(|destination| destination.target_dir(dirs).join(destination.target_name()))
        .collect()
}

/// Remove all files installed by the given manifest.
///
/// Returns all removed files.
#[throws]
pub fn remove_manifest(dirs: &mut InstallDirs, manifest: &Manifest) -> Vec<PathBuf> {
    let installed_files = files(dirs, manifest);
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
