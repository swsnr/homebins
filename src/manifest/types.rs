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

#[derive(Debug, PartialEq, Deserialize)]
pub struct Meta {
    pub name: String,
    pub version: String,
    pub url: String,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct VersionCheck {
    pub args: Vec<String>,
    pub pattern: String,
}

impl VersionCheck {
    pub fn regex(&self) -> std::result::Result<Regex, regex::Error> {
        Regex::new(&self.pattern)
    }
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Discover {
    pub binary: String,
    pub version_check: VersionCheck,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Checksums {
    pub b2: String,
}

#[derive(Debug, PartialEq, Deserialize)]
pub enum Shell {
    #[serde(rename = "fish")]
    Fish,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(tag = "type")]
pub enum Target {
    #[serde(rename = "binary", alias = "bin")]
    Binary,
    #[serde(rename = "manpage", alias = "man")]
    Manpage { section: u8 },
    #[serde(rename = "completion")]
    Completion { shell: Shell },
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct InstallFile {
    pub source: PathBuf,
    pub name: Option<String>,
    #[serde(flatten)]
    pub target: Target,
}

impl InstallFile {
    pub fn is_executable(&self) -> bool {
        match self.target {
            Target::Binary => true,
            _ => false,
        }
    }
}

fn deserialize_url<'de, D>(d: D) -> std::result::Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(d).and_then(|s| Url::parse(&s).map_err(serde::de::Error::custom))
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Install {
    #[serde(deserialize_with = "deserialize_url")]
    pub download: Url,
    pub checksums: Checksums,
    pub files: Vec<InstallFile>,
}

impl Install {
    #[throws]
    pub fn filename(&self) -> &str {
        self.download
            .path_segments()
            .ok_or(anyhow!("Expected path segments in URL {}", self.download))?
            // If there's a path there's also a last segment
            .last()
            .unwrap()
    }
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Manifest {
    pub meta: Meta,
    pub discover: Discover,
    pub install: Vec<Install>,
}

impl Manifest {
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
    fn deserialize_ripgrep_manifest() {
        let manifest = Manifest::read_from_path("manifests/ripgrep.toml").unwrap();
        assert_eq!(manifest, Manifest {
            meta: Meta {
                name: "ripgrep".to_string(),
                version: "12.1.1".to_string(),
                url: "https://github.com/BurntSushi/ripgrep".to_string(),
            },
            discover: Discover {
                binary: "rg".to_string(),
                version_check: VersionCheck {
                    args: vec!["--version".to_string()],
                    pattern: "ripgrep ([^ ]+)".to_string(),
                },
            },
            install: vec![
                Install {
                    download: Url::parse("https://github.com/BurntSushi/ripgrep/releases/download/12.1.1/ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz").unwrap(),
                    checksums: Checksums {
                        b2: "1c97a37e109f818bce8e974eb3a29eb8d1ca488e048caff658696211e8cad23728a767a2d6b97fed365d24f9545f1bc49a3e2687ab437eb4189993ad5fe30663".to_string()
                    },
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
            ],
        })
    }
}
