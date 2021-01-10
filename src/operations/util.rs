// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::types::{DestinationDirectory, Permissions};
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
