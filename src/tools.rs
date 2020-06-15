// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! External tools.

use std::ffi::OsString;
use std::io::Result;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::process::Command;

use url::Url;

use crate::process::CommandExt;

/// Get the manpath.
pub fn manpath() -> Result<OsString> {
    Ok(OsString::from_vec(
        Command::new("manpath").checked_output()?.stdout,
    ))
}

/// Download a URL with curl.
pub fn curl(url: &Url, target: &Path) -> Result<()> {
    Command::new("curl")
        .args(&[
            "-gqb",
            "",
            "-fLC",
            "-",
            "--progress-bar",
            "--retry",
            "3",
            "--retry-delay",
            "3",
        ])
        .arg("--output")
        .arg(target)
        .arg(url.as_str())
        .checked_call()
}

/// Newtype wrapper identifying an archive.
pub struct Archive<'a>(&'a Path);

pub fn untar(archive: Archive, target_directory: &Path) -> Result<()> {
    let Archive(archive) = archive;
    Command::new("tar")
        .arg("xf")
        .arg(archive)
        .arg("-C")
        .arg(target_directory)
        .checked_call()
}

pub fn unzip(archive: Archive, target_directory: &Path) -> Result<()> {
    let Archive(archive) = archive;
    Command::new("unzip")
        .arg(archive)
        .arg("-d")
        .arg(target_directory)
        .checked_call()
}

type ExtractFn = fn(Archive<'_>, &Path) -> Result<()>;

static ARCHIVE_PATTERNS: [(&str, ExtractFn); 5] = [
    (".tar.gz", untar),
    (".tgz", untar),
    (".tar.bz2", untar),
    (".tar.xz", untar),
    ("zip", unzip),
];

/// The result of attempting to extract an archive.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ArchiveResult {
    /// The archive was extracted.
    Extracted,
    /// The given file was not an archive.
    NoArchive,
}

/// Extract the given file if its an archive.
pub fn maybe_extract(file: &Path, directory: &Path) -> Result<ArchiveResult> {
    for (extension, extract) in &ARCHIVE_PATTERNS {
        if file.as_os_str().to_string_lossy().ends_with(extension) {
            extract(Archive(file), directory)?;
            return Ok(ArchiveResult::Extracted);
        }
    }
    Ok(ArchiveResult::NoArchive)
}

/// Create a git command for the given repo
pub fn git(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo);
    command
}
