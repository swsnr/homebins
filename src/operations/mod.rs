// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

mod types;

pub use types::*;

use crate::manifest::{Install, InstallDownload, Manifest, Target};
use std::borrow::Cow;
use std::borrow::Cow::Borrowed;

fn number_of_operations(download: &InstallDownload) -> usize {
    let no_files = match &download.install {
        Install::SingleFile { .. } => 1,
        Install::FilesFromArchive { files } => files.len(),
    };
    // Download and checksum validation
    no_files + 2
}

fn copy<'a>(source: Source<'a>, target: &Target, name: Cow<'a, str>) -> Operation<'a> {
    use Operation::Copy;
    let (dir, permissions) = match target {
        Target::Binary { .. } => (DestinationDirectory::BinDir, Permissions::Executable),
        Target::Manpage { section } => {
            (DestinationDirectory::ManDir(*section), Permissions::Regular)
        }
        Target::SystemdUserUnit => (
            DestinationDirectory::SystemdUserUnitDir,
            Permissions::Regular,
        ),
        Target::Completion { shell } => (
            DestinationDirectory::CompletionDir(*shell),
            Permissions::Regular,
        ),
    };
    Copy(source, Destination::new(dir, name), permissions)
}

fn add_links<'a>(target: &'a Target, target_name: &'a str, operations: &mut Vec<Operation<'a>>) {
    if let Target::Binary { links } = target {
        for link in links {
            operations.push(Operation::Hardlink(Cow::from(target_name), Cow::from(link)))
        }
    }
}

/// Create a list of operations necessary to install `manifest`.
pub fn install_manifest(manifest: &Manifest) -> Vec<Operation<'_>> {
    let number_of_operations = manifest.install.iter().map(number_of_operations).sum();
    let mut operations = Vec::with_capacity(number_of_operations);
    for download in &manifest.install {
        let filename = download.filename();
        operations.push(Operation::Download(
            Borrowed(&download.download),
            Borrowed(filename),
            Borrowed(&download.checksums),
        ));

        match &download.install {
            Install::SingleFile { name, target } => {
                let target_name = name.as_deref().unwrap_or(filename);
                operations.push(copy(
                    Source::new(SourceDirectory::Download, Cow::from(filename)),
                    target,
                    Cow::Borrowed(target_name),
                ));
                add_links(target, target_name, &mut operations);
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
                    add_links(&file.target, name, &mut operations);
                }
            }
        }
    }
    operations
}

/// Get a list of all installation destinations within `operations`.
pub fn operation_destinations<'a, I>(operations: I) -> Vec<Destination<'a>>
where
    I: Iterator<Item = &'a Operation<'a>>,
{
    let (min, max) = operations.size_hint();
    let mut destinations: Vec<Destination> = Vec::with_capacity(max.unwrap_or(min));
    for operation in operations {
        match operation {
            // TODO: Don't clone but always borrowed out of contained cows
            Operation::Copy(_, destination, _) => destinations.push(Destination::new(
                destination.directory(),
                Cow::from(destination.name()),
            )),
            Operation::Hardlink(_, target) => destinations.push(Destination::new(
                DestinationDirectory::BinDir,
                Cow::from(target.as_ref()),
            )),
            _ => {}
        }
    }
    destinations
}

#[cfg(test)]
mod tests {
    use crate::manifest::{Checksums, Shell};
    use crate::operations::DestinationDirectory::*;
    use crate::operations::SourceDirectory::*;
    use crate::operations::*;
    use crate::Manifest;
    use pretty_assertions::assert_eq;
    use std::borrow::Cow;
    use url::Url;

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

    #[test]
    fn install_destinations_all() {
        let operations = vec![
            Operation::Download(
                Cow::Owned(Url::parse("https://example.com/file.tar.gz").unwrap()),
                "file.tar.gz".into(),
                Cow::Owned(Checksums::default()),
            ),
            Operation::Copy(
                Source::new(WorkDir, "foo".into()),
                Destination::new(CompletionDir(Shell::Fish), "foo.fish".into()),
                Permissions::Regular,
            ),
            Operation::Copy(
                Source::new(WorkDir, "spam".into()),
                Destination::new(BinDir, "spam".into()),
                Permissions::Executable,
            ),
            Operation::Hardlink("spam".into(), "eggs".into()),
            Operation::Copy(
                Source::new(WorkDir, "spam.1".into()),
                Destination::new(ManDir(42), "spam.1".into()),
                Permissions::Regular,
            ),
        ];
        assert_eq!(
            operation_destinations(operations.iter()),
            vec![
                Destination::new(CompletionDir(Shell::Fish), "foo.fish".into()),
                Destination::new(BinDir, "spam".into()),
                Destination::new(BinDir, "eggs".into()),
                Destination::new(ManDir(42), "spam.1".into())
            ]
        );
    }
}
