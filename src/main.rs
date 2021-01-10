// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A command line tool to install binaries to $HOME.

#![deny(warnings, clippy::all, missing_docs)]

use colored::*;

use anyhow::{anyhow, Context, Error, Result};
use directories::BaseDirs;
use fehler::{throw, throws};
use homebins::{HomebinProjectDirs, HomebinRepos, InstallDirs, Manifest};
use std::path::{Path, PathBuf};

#[derive(Copy, Clone)]
enum Installed {
    All,
    Outdated,
}

#[derive(Copy, Clone)]
enum List {
    All,
    Installed(Installed),
}

struct Commands {
    dirs: HomebinProjectDirs,
    install_dirs: InstallDirs,
}

fn read_manifests<I: Iterator<Item = R>, R: AsRef<Path>>(filenames: I) -> Result<Vec<Manifest>> {
    filenames.map(Manifest::read_from_path).collect()
}

impl Commands {
    #[throws]
    fn new() -> Commands {
        let dirs = HomebinProjectDirs::open()?;
        let install_dirs = InstallDirs::from_base_dirs(
            &BaseDirs::new()
                .with_context(|| "Cannot determine base dirs for current user".to_string())?,
        )?;

        Commands { dirs, install_dirs }
    }

    fn repos(&self) -> HomebinRepos {
        HomebinRepos::open(&self.dirs)
    }

    #[throws]
    fn list_manifests<'a, I: Iterator<Item = &'a Manifest>>(&self, manifests: I, mode: List) {
        let mut failed = false;
        for manifest in manifests {
            match mode {
                List::All => println!(
                    "{}: {} – {} ({})",
                    manifest.info.name.bold(),
                    manifest.info.version,
                    manifest.info.url.blue(),
                    format!("{}", manifest.info.license).italic()
                ),
                List::Installed(Installed::All) => {
                    match homebins::installed_manifest_version(&self.install_dirs, &manifest) {
                        Ok(Some(version)) => {
                            println!("{} = {}", manifest.info.name.bold(), version)
                        }
                        Ok(None) => {}
                        Err(error) => {
                            failed = true;
                            println!(
                                "{} = {}",
                                manifest.info.name.bold(),
                                format!("failed: {:#}", error).red()
                            )
                        }
                    }
                }
                List::Installed(Installed::Outdated) => {
                    match homebins::outdated_manifest_version(&self.install_dirs, &manifest) {
                        Ok(Some(version)) => println!(
                            "{} = {} -> {}",
                            manifest.info.name.bold(),
                            format!("{}", version).red(),
                            format!("{}", manifest.info.version).bold().green()
                        ),
                        Ok(None) => {}
                        Err(error) => {
                            failed = true;
                            println!(
                                "{} = {}",
                                manifest.info.name.bold(),
                                format!("failed: {:#}", error).red()
                            )
                        }
                    }
                }
            }
        }
        if failed {
            throw!(anyhow!("Some version checks failed"));
        }
    }

    #[throws]
    fn list_files(&self, manifest: &Manifest, existing: bool, to_remove: bool) -> () {
        let files = if to_remove {
            homebins::files_to_remove(&self.install_dirs, manifest)
        } else {
            homebins::installed_files(&self.install_dirs, manifest)
        };
        for file in files {
            if !existing || file.exists() {
                println!("{}", file.display());
            }
        }
    }

    #[throws]
    fn install_manifest(&mut self, name: &str, manifest: &Manifest) -> () {
        println!("Installing {}", name.bold());
        homebins::install_manifest(&self.dirs, &mut self.install_dirs, manifest)?;
        println!("{}", format!("{} installed", name).green());
    }

    #[throws]
    fn remove_manifest(&mut self, name: &str, manifest: &Manifest) -> () {
        if homebins::installed_manifest_version(&self.install_dirs, manifest)?.is_some() {
            println!("Removing {}", name.bold());
            homebins::remove_manifest(&self.dirs, &mut self.install_dirs, manifest)?;
            println!("{}", format!("{} removed", name).yellow())
        }
    }

    #[throws]
    fn update_manifest(&mut self, name: &str, manifest: &Manifest) -> () {
        if homebins::outdated_manifest_version(&self.install_dirs, manifest)?.is_some() {
            println!("Updating {}", name.bold());
            // Install overwrites; we do not need to remove old files.
            homebins::install_manifest(&self.dirs, &mut self.install_dirs, manifest)?;
            println!("{}", format!("{} updated", name).green());
        }
    }

    pub fn list(&mut self, mode: List) -> Result<()> {
        let store = self.repos().manifest_store()?;
        // FIXME: Don't unwrap here!  (Still we can safely assume that a store only has valid manifests to some degree)
        let mut manifests: Vec<Manifest> = store.manifests()?.map(|m| m.unwrap()).collect();
        manifests.sort_by_cached_key(|m| m.info.name.to_string());
        self.list_manifests(manifests.iter(), mode)
    }

    #[throws]
    pub fn files(&mut self, names: Vec<String>, existing: bool, to_remove: bool) -> () {
        let store = self.repos().manifest_store()?;
        for name in names {
            let manifest = store
                .load_manifest(&name)?
                .ok_or_else(|| anyhow!("Binary {} not found", name))?;
            self.list_files(&manifest, existing, to_remove)?;
        }
    }

    #[throws]
    pub fn install(&mut self, names: Vec<String>) -> () {
        let store = self.repos().manifest_store()?;
        for name in names {
            let manifest = store
                .load_manifest(&name)?
                .ok_or_else(|| anyhow!("Binary {} not found", name))?;
            self.install_manifest(&name, &manifest)?;
        }
    }

    #[throws]
    pub fn remove(&mut self, names: Vec<String>) -> () {
        let store = self.repos().manifest_store()?;
        for name in names {
            let manifest = store
                .load_manifest(&name)?
                .ok_or_else(|| anyhow!("Binary {} not found", name))?;
            self.remove_manifest(&name, &manifest)?;
        }
    }

    #[throws]
    pub fn update(&mut self, names: Option<Vec<String>>) -> () {
        let store = self.repos().manifest_store()?;
        match names {
            None => {
                for manifest in store.manifests()? {
                    let manifest = manifest?;
                    self.update_manifest(&manifest.info.name, &manifest)?;
                }
            }
            Some(names) => {
                for name in names {
                    let manifest = store
                        .load_manifest(&name)?
                        .ok_or_else(|| anyhow!("Binary {} not found", name))?;
                    self.update_manifest(&name, &manifest)?;
                }
            }
        }
    }

    pub fn manifest_list(&self, filenames: Vec<PathBuf>, mode: List) -> Result<()> {
        self.list_manifests(read_manifests(filenames.iter())?.iter(), mode)
    }

    #[throws]
    pub fn manifest_files(&self, filenames: Vec<PathBuf>, existing: bool, to_remove: bool) -> () {
        for manifest in read_manifests(filenames.iter())? {
            self.list_files(&manifest, existing, to_remove)?
        }
    }

    #[throws]
    pub fn manifest_install(&mut self, filenames: Vec<PathBuf>) -> () {
        for filename in filenames {
            let manifest = Manifest::read_from_path(&filename)?;
            self.install_manifest(&filename.display().to_string(), &manifest)?;
        }
    }

    #[throws]
    pub fn manifest_remove(&mut self, filenames: Vec<PathBuf>) -> () {
        for filename in filenames {
            let manifest = Manifest::read_from_path(&filename)?;
            self.remove_manifest(&filename.display().to_string(), &manifest)?;
        }
    }

    #[throws]
    pub fn manifest_update(&mut self, filenames: Vec<PathBuf>) -> () {
        for filename in filenames {
            let manifest = Manifest::read_from_path(&filename)?;
            self.update_manifest(&filename.display().to_string(), &manifest)?;
        }
    }
}

#[allow(clippy::cognitive_complexity)]
fn process_args(matches: &clap::ArgMatches) -> anyhow::Result<()> {
    use clap::*;

    let mut commands = Commands::new()?;

    match matches.subcommand() {
        ("list", _) => commands.list(List::All),
        ("", _) => commands.list(List::Installed(Installed::All)),
        ("installed", _) => commands.list(List::Installed(Installed::All)),
        ("outdated", _) => commands.list(List::Installed(Installed::Outdated)),
        ("files", Some(m)) => commands.files(
            values_t!(m.values_of("name"), String).unwrap_or_else(|e| e.exit()),
            m.is_present("existing"),
            m.is_present("remove"),
        ),
        ("install", Some(m)) => {
            commands.install(values_t!(m.values_of("name"), String).unwrap_or_else(|e| e.exit()))
        }
        ("remove", Some(m)) => {
            commands.remove(values_t!(m.values_of("name"), String).unwrap_or_else(|e| e.exit()))
        }
        ("update", Some(m)) => {
            let names = if m.is_present("name") {
                Some(values_t!(m.values_of("name"), String).unwrap_or_else(|e| e.exit()))
            } else {
                None
            };
            commands.update(names)
        }
        ("manifest-list", Some(m)) => commands.manifest_list(
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
            List::All,
        ),
        ("manifest-installed", Some(m)) => commands.manifest_list(
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
            List::Installed(Installed::All),
        ),
        ("manifest-outdated", Some(m)) => commands.manifest_list(
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
            List::Installed(Installed::Outdated),
        ),
        ("manifest-files", Some(m)) => commands.manifest_files(
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
            m.is_present("existing"),
            m.is_present("remove"),
        ),
        ("manifest-install", Some(m)) => commands.manifest_install(
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
        ),
        ("manifest-remove", Some(m)) => commands.manifest_remove(
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
        ),
        ("manifest-update", Some(m)) => commands.manifest_update(
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
        ),
        (other, _) => Err(anyhow!("Unknown subcommand: {}", other)),
    }
}

fn main() {
    use clap::*;
    let app = app_from_crate!()
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::ColoredHelp)
        .subcommand(SubCommand::with_name("list").about("List available binaries"))
        .subcommand(SubCommand::with_name("installed").about("List installed binaries (default)"))
        .subcommand(SubCommand::with_name("outdated").about("List outdated binaries"))
        .subcommand(
            SubCommand::with_name("files")
                .about("List files of binary")
                .arg(
                    Arg::with_name("existing")
                        .short("e")
                        .long("existing")
                        .help("Only existing files"),
                )
                .arg(
                    Arg::with_name("remove")
                        .short("r")
                        .long("remove")
                        .help("List all files that would be removed"),
                )
                .arg(
                    Arg::with_name("name")
                        .required(true)
                        .multiple(true)
                        .help("Binaries to install"),
                ),
        )
        .subcommand(
            SubCommand::with_name("install")
                .about("Install binaries")
                .arg(
                    Arg::with_name("name")
                        .required(true)
                        .multiple(true)
                        .help("Binaries to install"),
                ),
        )
        .subcommand(
            SubCommand::with_name("remove")
                .about("Remove binaries")
                .arg(
                    Arg::with_name("name")
                        .required(true)
                        .multiple(true)
                        .help("Binaries to remove"),
                ),
        )
        .subcommand(
            SubCommand::with_name("update")
                .about("Update binaries")
                .arg(
                    Arg::with_name("name")
                        .multiple(true)
                        .help("Binaries to update (default to all outdated binaries)"),
                ),
        )
        .subcommand(
            SubCommand::with_name("manifest-list")
                .about("List info for given manifest files")
                .arg(
                    Arg::with_name("manifest-file")
                        .required(true)
                        .multiple(true)
                        .help("Manifest files"),
                ),
        )
        .subcommand(
            SubCommand::with_name("manifest-installed")
                .about("Show installed versions given manifest files")
                .arg(
                    Arg::with_name("manifest-file")
                        .required(true)
                        .multiple(true)
                        .help("Manifest files"),
                ),
        )
        .subcommand(
            SubCommand::with_name("manifest-outdated")
                .about("Show outdated versions of given manifest files")
                .arg(
                    Arg::with_name("manifest-file")
                        .required(true)
                        .multiple(true)
                        .help("Manifest files"),
                ),
        )
        .subcommand(
            SubCommand::with_name("manifest-files")
                .about("List files of a manifest")
                .arg(
                    Arg::with_name("existing")
                        .short("e")
                        .long("existing")
                        .help("Only existing files"),
                )
                .arg(
                    Arg::with_name("remove")
                        .short("r")
                        .long("remove")
                        .help("List all files that would be removed"),
                )
                .arg(
                    Arg::with_name("manifest-file")
                        .required(true)
                        .multiple(true)
                        .help("Manifest files"),
                ),
        )
        .subcommand(
            SubCommand::with_name("manifest-install")
                .about("Install given manifest files")
                .arg(
                    Arg::with_name("manifest-file")
                        .required(true)
                        .multiple(true)
                        .help("Manifest files"),
                ),
        )
        .subcommand(
            SubCommand::with_name("manifest-remove")
                .about("Remove given manifest files")
                .arg(
                    Arg::with_name("manifest-file")
                        .required(true)
                        .multiple(true)
                        .help("Manifest files"),
                ),
        )
        .subcommand(
            SubCommand::with_name("manifest-update")
                .about("Update given manifest files")
                .arg(
                    Arg::with_name("manifest-file")
                        .required(true)
                        .multiple(true)
                        .help("Manifest files"),
                ),
        );

    if let Err(error) = process_args(&app.get_matches()) {
        eprintln!("{}", format!("Error: {:#}", error).red().bold());
        std::process::exit(1)
    }
}
