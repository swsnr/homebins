// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::manifest::{Install, InstallDownload, Manifest, Target};
use std::borrow::Cow;
use std::borrow::Cow::Borrowed;

use super::types::*;

trait ApproxNumberOfOperations {
    fn approx_number_of_operations(&self) -> usize;
}

impl ApproxNumberOfOperations for Target {
    fn approx_number_of_operations(&self) -> usize {
        match self {
            Target::Binary { links } => links.len() + 1,
            _ => 1,
        }
    }
}

impl ApproxNumberOfOperations for InstallDownload {
    fn approx_number_of_operations(&self) -> usize {
        match &self.install {
            Install::SingleFile { target, .. } => target.approx_number_of_operations(),
            Install::FilesFromArchive { files } => files
                .iter()
                .map(|f| f.target.approx_number_of_operations())
                .sum(),
        }
    }
}

impl ApproxNumberOfOperations for Manifest {
    fn approx_number_of_operations(&self) -> usize {
        self.install
            .iter()
            .map(ApproxNumberOfOperations::approx_number_of_operations)
            .sum()
    }
}

fn copy<'a>(source: Source<'a>, target: &Target, name: Cow<'a, str>) -> InstallOperation<'a> {
    use InstallOperation::Copy;
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

fn push_links<'a>(
    target: &'a Target,
    target_name: &'a str,
    operations: &mut Vec<InstallOperation<'a>>,
) {
    if let Target::Binary { links } = target {
        for link in links {
            operations.push(InstallOperation::Hardlink(
                Cow::from(target_name),
                Cow::from(link),
            ))
        }
    }
}

/// Create a list of operations necessary to install `manifest`.
pub fn install_manifest(manifest: &Manifest) -> Vec<InstallOperation<'_>> {
    let mut operations = Vec::with_capacity(manifest.approx_number_of_operations());
    for download in &manifest.install {
        let filename = download.filename();
        operations.push(InstallOperation::Download(
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
                push_links(target, target_name, &mut operations);
            }
            Install::FilesFromArchive { files } => {
                operations.push(InstallOperation::Extract(Borrowed(filename)));
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
                    push_links(&file.target, name, &mut operations);
                }
            }
        }
    }
    operations
}

/// Get a list of all installation destinations within `operations`.
pub fn operation_destinations<'a, I>(operations: I) -> impl Iterator<Item = Destination<'a>>
where
    I: Iterator<Item = &'a InstallOperation<'a>> + 'a,
{
    operations.filter_map(|operation| {
        match operation {
            // TODO: Don't clone but always borrowed out of contained cows
            InstallOperation::Copy(_, destination, _) => Some(Destination::new(
                destination.directory(),
                Cow::from(destination.name()),
            )),
            InstallOperation::Hardlink(_, target) => Some(Destination::new(
                DestinationDirectory::BinDir,
                Cow::from(target.as_ref()),
            )),
            _ => None,
        }
    })
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
                InstallOperation::Download(
                    Cow::Borrowed(&manifest.install[0].download),
                    Cow::Borrowed("ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz"),
                    Cow::Borrowed(&manifest.install[0].checksums),
                ),
                InstallOperation::Extract(Cow::from(
                    "ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz"
                )),
                InstallOperation::Copy(
                    Source::new(
                        WorkDir,
                        Cow::from("ripgrep-12.1.1-x86_64-unknown-linux-musl/rg")
                    ),
                    Destination::new(BinDir, Cow::from("rg")),
                    Permissions::Executable
                ),
                InstallOperation::Hardlink(Cow::Borrowed("rg"), Cow::from("ripgrep")),
                InstallOperation::Copy(
                    Source::new(
                        WorkDir,
                        Cow::from("ripgrep-12.1.1-x86_64-unknown-linux-musl/doc/rg.1")
                    ),
                    Destination::new(ManDir(1), Cow::from("rg.1")),
                    Permissions::Regular
                ),
                InstallOperation::Copy(
                    Source::new(
                        WorkDir,
                        Cow::from("ripgrep-12.1.1-x86_64-unknown-linux-musl/complete/rg.fish")
                    ),
                    Destination::new(CompletionDir(Shell::Fish), Cow::from("rg.fish")),
                    Permissions::Regular
                ),
                InstallOperation::Copy(
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
                InstallOperation::Download(
                    Cow::Borrowed(&manifest.install[0].download),
                    Cow::from("shfmt_v3.1.1_linux_amd64"),
                    Cow::Borrowed(&manifest.install[0].checksums),
                ),
                InstallOperation::Copy(
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
            InstallOperation::Download(
                Cow::Owned(Url::parse("https://example.com/file.tar.gz").unwrap()),
                "file.tar.gz".into(),
                Cow::Owned(Checksums::default()),
            ),
            InstallOperation::Copy(
                Source::new(WorkDir, "foo".into()),
                Destination::new(CompletionDir(Shell::Fish), "foo.fish".into()),
                Permissions::Regular,
            ),
            InstallOperation::Copy(
                Source::new(WorkDir, "spam".into()),
                Destination::new(BinDir, "spam".into()),
                Permissions::Executable,
            ),
            InstallOperation::Hardlink("spam".into(), "eggs".into()),
            InstallOperation::Copy(
                Source::new(WorkDir, "spam.1".into()),
                Destination::new(ManDir(42), "spam.1".into()),
                Permissions::Regular,
            ),
        ];
        assert_eq!(
            operation_destinations(operations.iter()).collect::<Vec<Destination>>(),
            vec![
                Destination::new(CompletionDir(Shell::Fish), "foo.fish".into()),
                Destination::new(BinDir, "spam".into()),
                Destination::new(BinDir, "eggs".into()),
                Destination::new(ManDir(42), "spam.1".into())
            ]
        );
    }
}
