// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::install::*;
use super::remove::*;
use super::types::*;
use crate::Manifest;

/// Create operations to update the given manifest to a newer version.
pub fn update_manifest(manifest: &Manifest) -> Vec<Operation<'_>> {
    let mut operations = Vec::with_capacity(
        manifest.number_of_install_operations() + manifest.remove.additional_files.len(),
    );
    // Download all new artifacts first.
    for download in &manifest.install {
        push_download(download, &mut operations);
    }
    // Then remove legacy files.
    push_additional_remove(&manifest.remove, &mut operations);
    // Then install all files again, which overwrites those form the previous release
    for download in &manifest.install {
        push_download_install(download, &mut operations);
    }
    operations
}
