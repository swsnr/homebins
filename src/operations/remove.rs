// Copyright Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::install::install_manifest;
use super::types::*;
use super::util::*;
use crate::manifest::Remove;
use crate::Manifest;

pub fn push_additional_remove<'a>(remove: &'a Remove, operations: &mut Vec<Operation<'a>>) {
    for to_remove in &remove.additional_files {
        let (dir, _) = dir_and_permissions(&to_remove.target);
        operations.push(Operation::Remove(dir, (&to_remove.name).into()))
    }
}

/// Create a list of operations necessary to remove `manifest`.
pub fn remove_manifest(manifest: &Manifest) -> Vec<Operation<'_>> {
    let install_ops = install_manifest(manifest);
    let mut remove_ops =
        Vec::with_capacity(install_ops.len() + manifest.remove.additional_files.len());
    for destination in operation_destinations(install_ops.iter()) {
        remove_ops.push(Operation::Remove(
            destination.directory(),
            destination.name().to_string().into(),
        ));
    }
    push_additional_remove(&manifest.remove, &mut remove_ops);
    remove_ops
}
