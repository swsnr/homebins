// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::types::*;
use crate::manifest::Target;

pub fn dir_and_permissions(target: &Target) -> (DestinationDirectory, Permissions) {
    match target {
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
    }
}

/// Get a list of all installation destinations within `operations`.
pub fn operation_destinations<'a, I>(operations: I) -> impl Iterator<Item = Destination<'a>>
where
    I: Iterator<Item = &'a Operation<'a>> + 'a,
{
    operations.filter_map(|operation| {
        match operation {
            // TODO: Don't clone but always borrowed out of contained cows
            Operation::Copy(_, destination, _) => Some(Destination::new(
                destination.directory(),
                destination.name().into(),
            )),
            Operation::Hardlink(_, target) => Some(Destination::new(
                DestinationDirectory::BinDir,
                target.as_ref().into(),
            )),
            Operation::Remove(directory, name) => {
                Some(Destination::new(*directory, name.as_ref().into()))
            }
            Operation::Download(_, _, _) => None,
            Operation::Extract(_) => None,
        }
    })
}

#[cfg(tests)]
mod tests {
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
