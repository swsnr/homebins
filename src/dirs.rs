// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::manifest::Shell;
use crate::operations::{DestinationDirectory, SourceDirectory};
use crate::Manifest;
use anyhow::{Context, Result};
use directories::{BaseDirs, ProjectDirs};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

/// Get the project directories for homebins.
fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("de", "swsnr", "homebins")
        .with_context(|| "Failed to get home directory".to_string())
}

/// Homebin project dirs.
///
/// This struct provides the directories homebin uses for its own information.
///
/// In particular
#[derive(Debug)]
pub struct HomebinProjectDirs {
    repos_dir: PathBuf,
    download_dir: PathBuf,
}

impl HomebinProjectDirs {
    /// Open homebin project directories.
    pub fn open() -> Result<HomebinProjectDirs> {
        project_dirs().map(|dirs| HomebinProjectDirs {
            repos_dir: dirs.cache_dir().join("manifest_repos"),
            download_dir: dirs.cache_dir().join("downloads"),
        })
    }

    /// Get the directory for manifest repositories.
    pub fn repos_dir(&self) -> &Path {
        &self.repos_dir
    }

    /// Get the directory for manifest downloads.
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }

    /// The download directory for a specific manifest.
    ///
    /// This is a subdirectory of the download directory with the name and
    /// the version of the given manifest.
    pub fn manifest_download_dir(&self, manifest: &Manifest) -> PathBuf {
        self.download_dir
            .join(&manifest.info.name)
            .join(&manifest.info.version.to_string())
    }
}

/// Homebin directories.
///
/// This struct holds directories homebins installs to.
#[derive(Debug)]
pub struct InstallDirs {
    bin_dir: PathBuf,
    man_base_dir: PathBuf,
    systemd_user_unit_dir: PathBuf,
    fish_completion_dir: PathBuf,
}

impl InstallDirs {
    /// Determine installation directories from user base dirs.
    pub fn from_base_dirs(dirs: &BaseDirs) -> Result<InstallDirs> {
        Ok(InstallDirs {
            bin_dir: dirs
                .executable_dir()
                .with_context(|| {
                    "Cannot determine executable directory from base dirs".to_string()
                })?
                .to_path_buf(),
            man_base_dir: dirs.data_local_dir().join("man"),
            // According to systemd.unit(5) this is the place for units of packages installed to $HOME
            systemd_user_unit_dir: dirs.data_local_dir().join("systemd").join("user"),
            fish_completion_dir: dirs.config_dir().join("fish").join("completions"),
        })
    }

    /// The directory for binaries.
    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    /// The base directory for manpages.
    pub fn man_dir(&self) -> &Path {
        &self.man_base_dir
    }

    /// The directory to install man pages of the given section to.
    ///
    /// This is the corresponding sub-directory of the man_dir.
    pub fn man_section_dir(&self, section: u8) -> PathBuf {
        self.man_base_dir.join(format!("man{}", section))
    }

    /// The directory for systemd user units.
    pub fn systemd_user_unit_dir(&self) -> &Path {
        &self.systemd_user_unit_dir
    }

    /// The directory for completion files of the given `shell`.
    pub fn shell_completion_dir(&self, shell: Shell) -> &Path {
        match shell {
            Shell::Fish => &self.fish_completion_dir,
        }
    }

    /// Get the path for the given destination directory.
    pub fn path(&self, directory: DestinationDirectory) -> Cow<Path> {
        match directory {
            DestinationDirectory::BinDir => Cow::from(&self.bin_dir),
            DestinationDirectory::ManDir(section) => Cow::from(self.man_section_dir(section)),
            DestinationDirectory::SystemdUserUnitDir => Cow::from(&self.systemd_user_unit_dir),
            DestinationDirectory::CompletionDir(shell) => {
                Cow::from(self.shell_completion_dir(shell))
            }
        }
    }
}

/// Directories for operations of a single manifest.
#[derive(Debug)]
pub struct ManifestOperationDirs<'a> {
    install_dirs: &'a mut InstallDirs,
    download_dir: PathBuf,
    work_dir: TempDir,
}

impl<'a> ManifestOperationDirs<'a> {
    /// Create directories to apply operations of the given manifest.
    pub fn for_manifest(
        dirs: &HomebinProjectDirs,
        install_dirs: &'a mut InstallDirs,
        manifest: &Manifest,
    ) -> Result<ManifestOperationDirs<'a>> {
        tempdir()
            .with_context(|| {
                format!(
                    "Failed to create workdir for manifest {}",
                    manifest.info.name
                )
            })
            .map(move |work_dir| ManifestOperationDirs {
                work_dir,
                install_dirs,
                download_dir: dirs.manifest_download_dir(manifest),
            })
    }

    /// The directories to install to.
    pub fn install_dirs(&self) -> &InstallDirs {
        self.install_dirs
    }

    /// The directories to download files to.
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }

    /// The working directory to extract files to.
    pub fn work_dir(&self) -> &Path {
        &self.work_dir.path()
    }

    /// Get the path of the given source directory.
    pub fn path(&self, directory: SourceDirectory) -> &Path {
        match directory {
            SourceDirectory::Download => &self.download_dir,
            SourceDirectory::WorkDir => &self.work_dir.path(),
        }
    }

    /// Close these directories, i.e. delete the working directory.
    ///
    /// Also happens when dropped.
    pub fn close(self) -> Result<()> {
        self.work_dir
            .close()
            .with_context(|| "Failed to delete manifest workdir".to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::manifest::Shell;
    use crate::operations::DestinationDirectory;
    use crate::InstallDirs;
    use directories::BaseDirs;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn install_dirs_from_base_dirs() {
        std::env::set_var("XDG_CONFIG_HOME", "/test/config");
        std::env::set_var("XDG_DATA_HOME", "/test/data_home");
        std::env::set_var("XDG_BIN_DIR", "/test/bin");
        let dirs = InstallDirs::from_base_dirs(&BaseDirs::new().expect("base dirs"))
            .expect("install dirs");
        assert_eq!(
            dirs.path(DestinationDirectory::BinDir),
            Path::new("/test/bin")
        );
        assert_eq!(
            dirs.path(DestinationDirectory::ManDir(4)),
            Path::new("/test/data_home/man/man4")
        );
        assert_eq!(
            dirs.path(DestinationDirectory::SystemdUserUnitDir),
            Path::new("/test/data_home/systemd/user")
        );
        assert_eq!(
            dirs.path(DestinationDirectory::CompletionDir(Shell::Fish)),
            Path::new("/test/config/fish/completions")
        );
    }
}
