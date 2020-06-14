// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Manifest types.

use anyhow::{anyhow, Context, Error, Result};
use fehler::throws;
use regex::Regex;
use serde::{Deserialize, Deserializer};
use std::path::{Path, PathBuf};
use url::Url;
use versions::Versioning;

fn deserialize_versioning<'de, D>(d: D) -> std::result::Result<Versioning, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(d).and_then(|s| {
        Versioning::new(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("Invalid version: {:?}", s)))
    })
}

fn deserialize_spdx<'de, D>(d: D) -> std::result::Result<spdx::Expression, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(d)
        .and_then(|s| spdx::Expression::parse(&s).map_err(serde::de::Error::custom))
}

/// Information about the binary in this manifest.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Info {
    /// The name of the binary.
    pub name: String,
    /// The version of the binary this manifest describes.
    #[serde(deserialize_with = "deserialize_versioning")]
    pub version: Versioning,
    /// An URL for this binary, i.e. its website.
    pub url: String,
    #[serde(deserialize_with = "deserialize_spdx", alias = "licence")]
    /// The license of this binary.
    ///
    /// This is an SPDX expression describing the licenses this binary is distributed under.
    pub license: spdx::Expression,
}

/// How to check the version of a binary.
#[derive(Debug, PartialEq, Deserialize)]
pub struct VersionCheck {
    /// The arguments to pass to the binary to make it output its version.
    pub args: Vec<String>,
    /// A regular expression to extract the version from the binary invoked with `args`.
    pub pattern: String,
}

impl VersionCheck {
    /// Create a regex from the `pattern`.
    pub fn regex(&self) -> std::result::Result<Regex, regex::Error> {
        Regex::new(&self.pattern)
    }
}

/// How to check whether a binary exists.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Discover {
    /// The name of the binary to look for.
    ///
    /// Just the file name in `$HOME/.local/bin`.
    pub binary: String,
    /// How to check the version of this binary.
    pub version_check: VersionCheck,
}

fn deserialize_hex<'de, D>(d: D) -> std::result::Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(d).and_then(|v| {
        v.map(|s| hex::decode(s).map_err(serde::de::Error::custom))
            .transpose()
    })
}

/// Checksums for validation of downloads.
#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct Checksums {
    /// A Blake2 checksum.
    #[serde(deserialize_with = "deserialize_hex", default)]
    pub b2: Option<Vec<u8>>,
    /// A SHA512 checksum.
    #[serde(deserialize_with = "deserialize_hex", default)]
    pub sha512: Option<Vec<u8>>,
    /// A SHA256 checksum.
    #[serde(deserialize_with = "deserialize_hex", default)]
    pub sha256: Option<Vec<u8>>,
    /// A SHA1 checksum.
    #[serde(deserialize_with = "deserialize_hex", default)]
    pub sha1: Option<Vec<u8>>,
}

impl Checksums {
    /// Whether these checksums are empty.
    pub fn is_empty(&self) -> bool {
        if let Checksums {
            b2: None,
            sha512: None,
            sha256: None,
            sha1: None,
        } = self
        {
            true
        } else {
            false
        }
    }
}

/// Known shells.
#[derive(Debug, PartialEq, Deserialize, Copy, Clone)]
pub enum Shell {
    /// The Fish shell.
    #[serde(rename = "fish")]
    Fish,
}

/// The kind of installation target.
#[derive(Debug, PartialEq, Deserialize, Copy, Clone)]
#[serde(tag = "type")]
pub enum Target {
    /// A binary to install to `$HOME/.local/bin` as executable.
    #[serde(rename = "binary", alias = "bin")]
    Binary,
    /// A manpage to install at the given secion in `$HOME/.local/share/man` as regular file.
    #[serde(rename = "manpage", alias = "man")]
    Manpage {
        /// The section of this manpage, from 1 to 9.
        section: u8,
    },
    /// An tab completion helper for a shell.
    #[serde(rename = "completion")]
    Completion {
        /// The shell to install this completion file for.
        shell: Shell,
    },
}

impl Target {
    /// Whether this file needs to be installed as executable.
    pub fn is_executable(self) -> bool {
        match self {
            Target::Binary => true,
            _ => false,
        }
    }
}

/// A file to install to $HOME.
#[derive(Debug, PartialEq, Deserialize)]
pub struct InstallFile {
    /// The path of this file within the containing download.
    pub source: PathBuf,
    /// An explicit file name to install as.
    ///
    /// If absent use the file name of `source`.
    pub name: Option<String>,
    /// The target to install the file as.
    #[serde(flatten)]
    pub target: Target,
}

fn deserialize_url<'de, D>(d: D) -> std::result::Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(d).and_then(|s| Url::parse(&s).map_err(serde::de::Error::custom))
}

/// What to install from a download.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Install {
    /// Install the downloaded file directly as a single file.
    SingleFile {
        /// An explicit file name to install as.
        ///
        /// If absent use the file name of the download.
        name: Option<String>,
        /// The target to install the file as.
        #[serde(flatten)]
        target: Target,
    },
    /// Install files extracted from a downloaded archive.
    FilesFromArchive {
        /// A list of files to install.
        files: Vec<InstallFile>,
    },
}

fn deserialize_and_validate_checksums<'de, D>(d: D) -> std::result::Result<Checksums, D::Error>
where
    D: Deserializer<'de>,
{
    Checksums::deserialize(d).and_then(|checksums| {
        if checksums.is_empty() {
            Err(serde::de::Error::custom("No checksums given"))
        } else {
            Ok(checksums)
        }
    })
    // String::deserialize(d).and_then(|s| Url::parse(&s).map_err(serde::de::Error::custom))
}

/// An installation definition.
///
/// A URL to download, extract if required, and install to $HOME.
#[derive(Debug, PartialEq, Deserialize)]
pub struct InstallDownload {
    /// The URL to download from.
    #[serde(deserialize_with = "deserialize_url")]
    pub download: Url,
    /// Checksums to verify the download with.
    #[serde(deserialize_with = "deserialize_and_validate_checksums")]
    pub checksums: Checksums,
    /// Files to install from this download.
    #[serde(flatten)]
    pub install: Install,
}

impl InstallDownload {
    /// The file name of the URL, that is, the final segment of the path of `download`.
    #[throws]
    pub fn filename(&self) -> &str {
        self.download
            .path_segments()
            .ok_or_else(|| anyhow!("Expected path segments in URL {}", self.download))?
            // If there's a path there's also a last segment
            .last()
            .unwrap()
    }
}

/// A manifest describing an installable binary.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Manifest {
    /// Information about this binary.
    pub info: Info,
    /// How to discover whether this binary already exists.
    pub discover: Discover,
    /// A list of install steps to install this binary.
    pub install: Vec<InstallDownload>,
}

impl Manifest {
    /// Read a manifest from the file denoted by the given `path`.
    pub fn read_from_path<P: AsRef<Path>>(path: P) -> Result<Manifest> {
        toml::from_str(&std::fs::read_to_string(path.as_ref())?)
            .with_context(|| format!("File {} is no valid manifest", path.as_ref().display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn deserialize_manifest_with_files() {
        let manifest = Manifest::read_from_path("tests/manifests/ripgrep.toml").unwrap();
        assert_eq!(manifest, Manifest {
            info: Info {
                name: "ripgrep".to_string(),
                version: Versioning::new("12.1.1").unwrap(),
                url: "https://github.com/BurntSushi/ripgrep".to_string(),
                license: spdx::Expression::parse("Unlicense OR MIT").unwrap(),
            },
            discover: Discover {
                binary: "rg".to_string(),
                version_check: VersionCheck {
                    args: vec!["--version".to_string()],
                    pattern: "ripgrep ([^ ]+)".to_string(),
                },
            },
            install: vec![
                InstallDownload {
                    download: Url::parse("https://github.com/BurntSushi/ripgrep/releases/download/12.1.1/ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz").unwrap(),
                    checksums: Checksums {
                        b2: Some(hex::decode("1c97a37e109f818bce8e974eb3a29eb8d1ca488e048caff658696211e8cad23728a767a2d6b97fed365d24f9545f1bc49a3e2687ab437eb4189993ad5fe30663").unwrap()),
                        ..Checksums::default()
                    },
                    install: Install::FilesFromArchive {
                        files: vec![
                            InstallFile {
                                source: Path::new("ripgrep-12.1.1-x86_64-unknown-linux-musl/rg").to_path_buf(),
                                name: None,
                                target: Target::Binary,
                            },
                            InstallFile {
                                source: Path::new("ripgrep-12.1.1-x86_64-unknown-linux-musl/doc/rg.1").to_path_buf(),
                                name: None,
                                target: Target::Manpage { section: 1 },
                            },
                            InstallFile {
                                source: Path::new("ripgrep-12.1.1-x86_64-unknown-linux-musl/complete/rg.fish").to_path_buf(),
                                name: None,
                                target: Target::Completion { shell: Shell::Fish },
                            }
                        ],
                    }
                }
            ],
        })
    }

    #[test]
    fn deserialize_manifest_with_single_file() {
        let manifest = Manifest::read_from_path("tests/manifests/shfmt.toml").unwrap();
        assert_eq!(
            manifest,
            Manifest {
                info: Info {
                    name: "shfmt".to_string(),
                    version: Versioning::new("3.1.1").unwrap(),
                    url: "https://github.com/mvdan/sh".to_string(),
                    license: spdx::Expression::parse("BSD-3-Clause").unwrap()
                },
                discover: Discover {
                    binary: "shfmt".to_string(),
                    version_check: VersionCheck {
                        args: vec!["-version".to_string()],
                        pattern: "v(\\d\\S+)".to_string()
                    }
                },
                install: vec![InstallDownload {
                    download: Url::parse("https://github.com/mvdan/sh/releases/download/v3.1.1/shfmt_v3.1.1_linux_amd64").unwrap(),
                    checksums: Checksums {
                        b2: Some(hex::decode("15b203be254ca46b25d35654ceaae91b7e9200f49cd81e103eae7dd80d9e73ab4455c33e6f20073ba2b45f93b06e94e46556c1ab619812718185e071576cf48c").unwrap()),
                        ..Checksums::default()
                    },
                    install: Install::SingleFile {
                        name: Some("shfmt".to_string()),
                        target: Target::Binary
                    }
                }]
            }
        )
    }
}
