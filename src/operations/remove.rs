// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::install::{install_manifest, operation_destinations};
use super::types::*;
use super::util::*;
use crate::Manifest;

/// Create a list of operations necessary to remove `manifest`.
pub fn remove_manifest(manifest: &Manifest) -> Vec<RemoveOperation<'_>> {
    let install_ops = install_manifest(manifest);
    let mut remove_ops = Vec::with_capacity(install_ops.len());
    for destination in operation_destinations(install_ops.iter()) {
        remove_ops.push(RemoveOperation::Delete(
            destination.directory(),
            destination.name().to_string().into(),
        ));
    }
    for to_remove in &manifest.remove.additional_files {
        let (dir, _) = dir_and_permissions(&to_remove.target);
        remove_ops.push(RemoveOperation::Delete(dir, (&to_remove.name).into()))
    }
    remove_ops
}
