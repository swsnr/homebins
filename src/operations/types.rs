// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::manifest::{Checksums, Shell};
use crate::InstallDirs;
use std::borrow::Cow;
use std::path::Path;
use url::Url;

/// A source for a copy operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source<'a> {
    /// A downloaded file in the manifest download directory.
    Download(Cow<'a, str>),
    /// A file path in the manifest work directory.
    WorkDir(Cow<'a, str>),
}

/// The destination of a copy operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination<'a> {
    /// Install to the binary dir at the given name.
    BinDir(Cow<'a, str>),
    /// Install to the man dir for the given section at the given name.
    ManDir(u8, Cow<'a, str>),
    /// Install to the completions directory for the given shell at the given name.
    CompletionDir(Shell, Cow<'a, str>),
}

impl Destination<'_> {
    /// Get the target directory for this destination within `dirs`.
    pub fn target_dir<'a>(&self, dirs: &'a InstallDirs) -> Cow<'a, Path> {
        match *self {
            Destination::BinDir(_) => Cow::from(dirs.bin_dir()),
            Destination::ManDir(section, _) => Cow::from(dirs.man_section_dir(section)),
            Destination::CompletionDir(shell, _) => dirs.shell_completion_dir(shell),
        }
    }

    /// Get the target name for this destination.
    pub fn target_name(&self) -> &str {
        match self {
            Destination::BinDir(ref name) => name,
            Destination::ManDir(_, ref name) => name,
            Destination::CompletionDir(_, ref name) => name,
        }
    }
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

/// Operations to apply a manifest to a home directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation<'a> {
    /// Download a to the given filename in the manifest download directory.
    Download(Cow<'a, Url>, Cow<'a, str>),
    /// Validate checksums of the given file in the manifest download directory.
    Validate(Cow<'a, Checksums>, Cow<'a, str>),
    /// Extract the given filename from the manifest download directory into the manifest work directory.
    Extract(Cow<'a, str>),
    /// Copy the given source file to the given destination, with the given permissions on target.
    Copy(Source<'a>, Destination<'a>, Permissions),
    /// Create a hard link, from the first to the second item.
    Hardlink(Cow<'a, str>, Cow<'a, str>),
}
