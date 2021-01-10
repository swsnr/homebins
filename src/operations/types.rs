// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::manifest::{Checksums, Shell};
use std::borrow::Cow;
use std::ops::Deref;
use url::Url;

/// A source directory for manifest installation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SourceDirectory {
    /// The download directory of a manifest.
    Download,
    /// The working directory during manifest installation.
    ///
    /// This directory contains files from extracted archives.
    WorkDir,
}

/// The target directory for a copy operation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DestinationDirectory {
    /// The directory for binaries.
    BinDir,
    /// The directory for manpages of the given section.
    ManDir(u8),
    /// The directory for systemd user units.
    SystemdUserUnitDir,
    /// The directory for completion files for the given shell.
    CompletionDir(Shell),
}

/// Permissions for the target of a copy operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permissions {
    /// Permissions of a regular file (readable, and owner-writable)
    Regular,
    /// Permissions of an executable file (readable, owner-writable, and executable)
    Executable,
}

impl Permissions {
    /// Convert permissions to a Unix file mode.
    fn to_mode(self) -> u32 {
        use Permissions::*;
        match self {
            Regular => 0o644,
            Executable => 0o755,
        }
    }

    /// Convert these abstract permissions to concrete Unix filesystem permissions.
    pub fn to_unix_permissions(self) -> std::fs::Permissions {
        use std::os::unix::fs::PermissionsExt;
        std::fs::Permissions::from_mode(self.to_mode())
    }
}

/// The source or destination of a copy operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyOperand<'a, D> {
    /// The directory to copy from or to.
    directory: D,
    /// The name of the file to copy.
    name: Cow<'a, str>,
}

impl<'a, D> CopyOperand<'a, D> {
    /// Create a new copy operand.
    pub fn new(directory: D, name: Cow<'a, str>) -> Self {
        CopyOperand { directory, name }
    }

    /// The name of the file to copy.
    pub fn name(&self) -> &str {
        self.name.deref()
    }
}

impl<'a, D> CopyOperand<'a, D>
where
    D: Copy,
{
    /// The directory to copy from or to.
    pub fn directory(&self) -> D {
        self.directory
    }
}

/// The source of a copy operation.
pub type Source<'a> = CopyOperand<'a, SourceDirectory>;
/// The destination of a copy operation.
pub type Destination<'a> = CopyOperand<'a, DestinationDirectory>;

/// Operations to apply a manifest to a home directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallOperation<'a> {
    /// Download a to the given filename in the manifest download directory and validate against checksums.
    Download(Cow<'a, Url>, Cow<'a, str>, Cow<'a, Checksums>),
    /// Extract the given filename from the manifest download directory into the manifest work directory.
    Extract(Cow<'a, str>),
    /// Copy the given source file to the given destination, with the given permissions on target.
    Copy(Source<'a>, Destination<'a>, Permissions),
    /// Create a hard link, from the first to the second item.
    Hardlink(Cow<'a, str>, Cow<'a, str>),
}

/// Operations to remove a manifest from a home directory
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveOperation<'a> {
    /// Delete a file with the given name from the given destination directory.
    Delete(DestinationDirectory, Cow<'a, str>),
}
