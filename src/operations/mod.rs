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
    match target {
        Target::Binary { .. } => Copy(source, Destination::BinDir(name), Permissions::Executable),
        Target::Manpage { section } => Copy(
            source,
            Destination::ManDir(*section, name),
            Permissions::Regular,
        ),
        Target::Completion { shell } => Copy(
            source,
            Destination::CompletionDir(*shell, name),
            Permissions::Regular,
        ),
    }
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
        ));
        operations.push(Operation::Validate(
            Borrowed(&download.checksums),
            Borrowed(filename),
        ));

        match &download.install {
            Install::SingleFile { name, target } => {
                let target_name = name.as_deref().unwrap_or(filename);
                operations.push(copy(
                    Source::Download(Borrowed(filename)),
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
                        Source::WorkDir(Borrowed(file.source.as_str())),
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

#[cfg(test)]
mod tests {
    use crate::manifest::Shell;
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
                    Cow::Borrowed("ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz")
                ),
                Operation::Validate(
                    Cow::Borrowed(&manifest.install[0].checksums),
                    Cow::Borrowed("ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz")
                ),
                Operation::Extract(Cow::Borrowed(
                    "ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz"
                )),
                Operation::Copy(
                    Source::WorkDir(Cow::Borrowed("ripgrep-12.1.1-x86_64-unknown-linux-musl/rg")),
                    Destination::BinDir(Cow::Borrowed("rg")),
                    Permissions::Executable
                ),
                Operation::Hardlink(Cow::Borrowed("rg"), Cow::Borrowed("ripgrep")),
                Operation::Copy(
                    Source::WorkDir(Cow::Borrowed(
                        "ripgrep-12.1.1-x86_64-unknown-linux-musl/doc/rg.1"
                    )),
                    Destination::ManDir(1, Cow::Borrowed("rg.1")),
                    Permissions::Regular
                ),
                Operation::Copy(
                    Source::WorkDir(Cow::Borrowed(
                        "ripgrep-12.1.1-x86_64-unknown-linux-musl/complete/rg.fish"
                    )),
                    Destination::CompletionDir(Shell::Fish, Cow::Borrowed("rg.fish")),
                    Permissions::Regular
                ),
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
                    Cow::Borrowed("shfmt_v3.1.1_linux_amd64")
                ),
                Operation::Validate(
                    Cow::Borrowed(&manifest.install[0].checksums),
                    Cow::Borrowed("shfmt_v3.1.1_linux_amd64")
                ),
                Operation::Copy(
                    Source::Download(Cow::Borrowed("shfmt_v3.1.1_linux_amd64")),
                    Destination::BinDir(Cow::Borrowed("shfmt")),
                    Permissions::Executable
                )
            ]
        );
    }
}
