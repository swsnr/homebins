// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Deserialize)]
pub struct Meta {
    version: String,
    url: String,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct VersionCheck {
    args: Vec<String>,
    // TODO: Deserialize as regex
    pattern: String,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Discover {
    binary: String,
    version_check: VersionCheck,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Checksums {
    b2: String,
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
    #[serde(rename = "manpage")]
    Manpage { section: u8 },
    #[serde(rename = "completion")]
    Completion { shell: Shell },
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct InstallFile {
    source: PathBuf,
    target_name: Option<String>,
    #[serde(flatten)]
    target: Target,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Install {
    download: String,
    checksums: Checksums,
    files: Vec<InstallFile>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub struct Manifest {
    meta: Meta,
    discover: Discover,
    install: Vec<Install>,
}

impl Manifest {
    pub fn from_slice(s: &[u8]) -> Result<Manifest, toml::de::Error> {
        toml::from_slice(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    #[test]
    fn deserialize_ripgrep_manifest() {
        let mut buffer: Vec<u8> = Vec::new();
        File::open("manifests/ripgrep.toml")
            .unwrap()
            .read_to_end(&mut buffer)
            .unwrap();
        let manifest = Manifest::from_slice(&buffer).unwrap();
        assert_eq!(manifest, Manifest {
            meta: Meta {
                version: "12.1.1".to_string(),
                url: "https://github.com/BurntSushi/ripgrep".to_string(),
            },
            discover: Discover {
                binary: "rg".to_string(),
                version_check: VersionCheck {
                    args: vec!["--version".to_string()],
                    pattern: String::new(),
                },
            },
            install: vec![
                Install {
                    download: "https://github.com/BurntSushi/ripgrep/releases/download/12.1.1/ripgrep-12.1.1-x86_64-unknown-linux-musl.tar.gz".to_string(),
                    checksums: Checksums {
                        b2: "1c97a37e109f818bce8e974eb3a29eb8d1ca488e048caff658696211e8cad23728a767a2d6b97fed365d24f9545f1bc49a3e2687ab437eb4189993ad5fe30663".to_string()
                    },
                    files: vec![
                        InstallFile {
                            source: Path::new("ripgrep-12.1.1-x86_64-unknown-linux-musl/rg").to_path_buf(),
                            target_name: None,
                            target: Target::Binary,
                        },
                        InstallFile {
                            source: Path::new("ripgrep-12.1.1-x86_64-unknown-linux-musl/doc/rg.1").to_path_buf(),
                            target_name: None,
                            target: Target::Manpage { section: 1 },
                        },
                        InstallFile {
                            source: Path::new("ripgrep-12.1.1-x86_64-unknown-linux-musl/complete/rg.fish").to_path_buf(),
                            target_name: None,
                            target: Target::Completion { shell: Shell::Fish },
                        }
                    ],
                }
            ],
        })
    }
}
