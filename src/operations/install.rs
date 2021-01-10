// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::manifest::{Install, InstallDownload, Manifest, Target};
use std::borrow::Cow;
use std::borrow::Cow::Borrowed;

use super::types::*;
use super::util::*;

pub trait NumberOfInstallOperations {
    fn number_of_install_operations(&self) -> usize;
}

impl NumberOfInstallOperations for Target {
    fn number_of_install_operations(&self) -> usize {
        match self {
            Target::Binary { links } => links.len() + 1,
            _ => 1,
        }
    }
}

impl NumberOfInstallOperations for InstallDownload {
    fn number_of_install_operations(&self) -> usize {
        match &self.install {
            Install::SingleFile { target, .. } => target.number_of_install_operations(),
            Install::FilesFromArchive { files } => files
                .iter()
                .map(|f| f.target.number_of_install_operations())
                .sum(),
        }
    }
}

impl NumberOfInstallOperations for Manifest {
    fn number_of_install_operations(&self) -> usize {
        self.install
            .iter()
            .map(NumberOfInstallOperations::number_of_install_operations)
            .sum()
    }
}

fn copy<'a>(source: Source<'a>, target: &Target, name: Cow<'a, str>) -> Operation<'a> {
    use Operation::Copy;
    let (dir, permissions) = dir_and_permissions(target);
    Copy(source, Destination::new(dir, name), permissions)
}

fn push_links<'a>(target: &'a Target, target_name: &'a str, operations: &mut Vec<Operation<'a>>) {
    if let Target::Binary { links } = target {
        for link in links {
            operations.push(Operation::Hardlink(Cow::from(target_name), Cow::from(link)))
        }
    }
}

/// Add install operations of a given `download` to `operations`.
pub fn push_download_install<'a>(
    download: &'a InstallDownload,
    operations: &mut Vec<Operation<'a>>,
) {
    let filename = download.filename();
    match &download.install {
        Install::SingleFile { name, target } => {
            let target_name = name.as_deref().unwrap_or(filename);
            operations.push(copy(
                Source::new(SourceDirectory::Download, Cow::from(filename)),
                target,
                Cow::Borrowed(target_name),
            ));
            push_links(target, target_name, operations);
        }
        Install::FilesFromArchive { files } => {
            operations.push(Operation::Extract(Borrowed(filename)));
            for file in files {
                let name = file.name.as_deref().unwrap_or_else(|| {
                    file.source
                        .split('/')
                        .last()
                        .expect("rsplit should always be non-empty!")
                });
                operations.push(copy(
                    Source::new(SourceDirectory::WorkDir, Cow::from(file.source.as_str())),
                    &file.target,
                    Cow::from(name),
                ));
                push_links(&file.target, name, operations);
            }
        }
    }
}

/// Add the download operation of `download` to `operations`.
pub fn push_download<'a>(download: &'a InstallDownload, operations: &mut Vec<Operation<'a>>) {
    operations.push(Operation::Download(
        Borrowed(&download.download),
        Borrowed(download.filename()),
        Borrowed(&download.checksums),
    ));
}

/// Create a list of operations necessary to install `manifest`.
pub fn install_manifest(manifest: &Manifest) -> Vec<Operation<'_>> {
    let mut operations = Vec::with_capacity(manifest.number_of_install_operations());
    for download in &manifest.install {
        push_download(download, &mut operations);
        push_download_install(download, &mut operations);
    }
    operations
}

#[cfg(test)]
mod tests {
    use crate::manifest::Shell;
    use crate::operations::DestinationDirectory::*;
    use crate::operations::SourceDirectory::*;
    use crate::operations::*;
    use crate::Manifest;
    use pretty_assertions::assert_eq;
    use std::borrow::Cow;

    #[test]
    fn install_manifest_multiple_files() {
        let manifest = Manifest::read_from_path("tests/manifests/ripgrep.toml").unwrap();
        assert_eq!(
            install_manifest(&manifest),
            vec![
                Operation::Download(
                    Cow::Borrowed(&manifest.install[0].download),
                    Cow::Borrowed("ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz"),
                    Cow::Borrowed(&manifest.install[0].checksums),
                ),
                Operation::Extract(Cow::from("ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz")),
                Operation::Copy(
                    Source::new(
                        WorkDir,
                        Cow::from("ripgrep-12.1.1-x86_64-unknown-linux-musl/rg")
                    ),
                    Destination::new(BinDir, Cow::from("rg")),
                    Permissions::Executable
                ),
                Operation::Hardlink(Cow::Borrowed("rg"), Cow::from("ripgrep")),
                Operation::Copy(
                    Source::new(
                        WorkDir,
                        Cow::from("ripgrep-12.1.1-x86_64-unknown-linux-musl/doc/rg.1")
                    ),
                    Destination::new(ManDir(1), Cow::from("rg.1")),
                    Permissions::Regular
                ),
                Operation::Copy(
                    Source::new(
                        WorkDir,
                        Cow::from("ripgrep-12.1.1-x86_64-unknown-linux-musl/complete/rg.fish")
                    ),
                    Destination::new(CompletionDir(Shell::Fish), Cow::from("rg.fish")),
                    Permissions::Regular
                ),
                Operation::Copy(
                    Source::new(
                        WorkDir,
                        Cow::from("ripgrep-12.1.1-x86_64-unknown-linux-musl/rg.unit")
                    ),
                    Destination::new(SystemdUserUnitDir, Cow::from("rg.unit")),
                    Permissions::Regular
                )
            ]
        );
    }

    #[test]
    fn install_manifest_single_file() {
        let manifest = Manifest::read_from_path("tests/manifests/shfmt.toml").unwrap();
        assert_eq!(
            install_manifest(&manifest),
            vec![
                Operation::Download(
                    Cow::Borrowed(&manifest.install[0].download),
                    Cow::from("shfmt_v3.1.1_linux_amd64"),
                    Cow::Borrowed(&manifest.install[0].checksums),
                ),
                Operation::Copy(
                    Source::new(Download, Cow::from("shfmt_v3.1.1_linux_amd64")),
                    Destination::new(BinDir, Cow::from("shfmt")),
                    Permissions::Executable
                )
            ]
        );
    }
}
