// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::fs::File;
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, Error};
use colored::Colorize;
use fehler::throws;

use crate::checksum::Validate;
use crate::operations::Operation;
use crate::tools::{curl, extract};
use crate::ManifestOperationDirs;

/// Define application of operations.
pub trait ApplyOperation {
    /// Errors from applying operations.
    type Error;

    /// Apply this operation to the given manifest directories.
    fn apply_operation<'a>(&self, dirs: &ManifestOperationDirs<'a>) -> Result<(), Self::Error>;
}

impl<'a> ApplyOperation for Operation<'a> {
    type Error = anyhow::Error;

    #[throws]
    fn apply_operation<'b>(&self, dirs: &ManifestOperationDirs<'b>) -> () {
        use Operation::*;
        match self {
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
                let source_path = dirs.path(source.directory()).join(source.name());
                let target_dir = dirs.install_dirs().path(destination.directory());
                let target = target_dir.join(destination.name());
                println!(
                    "install -m{:o} {} {}",
                    mode,
                    source.name(),
                    target.display()
                );
                std::fs::create_dir_all(&target_dir)?;
                let mut temp_target = tempfile::Builder::new()
                    .prefix(destination.name())
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
            Remove(directory, name) => {
                let file = dirs.install_dirs().path(*directory).join(name.as_ref());
                println!("rm -f {}", file.display());
                if file.exists() {
                    std::fs::remove_file(&file)
                        .with_context(|| format!("Failed to remove {}", file.display()))?;
                }
            }
        }
    }
}
