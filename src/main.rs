// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A command line tool to install binaries to $HOME.

#![deny(warnings, clippy::all, missing_docs)]

use clap::*;
use colored::*;

use anyhow::anyhow;
use homebins::Home;
use std::path::PathBuf;

mod subcommands {
    use anyhow::{anyhow, Error, Result};
    use colored::*;
    use fehler::{throw, throws};
    use homebins::{Home, Manifest};
    use std::path::{Path, PathBuf};

    #[derive(Copy, Clone)]
    pub enum Installed {
        All,
        Outdated,
    }

    #[derive(Copy, Clone)]
    pub enum List {
        All,
        Installed(Installed),
    }

    #[throws]
    fn list_manifests<I: Iterator<Item = Manifest>>(home: &Home, manifests: I, mode: List) {
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
                List::Installed(Installed::All) => match home.installed_manifest_version(&manifest)
                {
                    Ok(Some(version)) => println!("{} = {}", manifest.info.name.bold(), version),
                    Ok(None) => {}
                    Err(error) => {
                        failed = true;
                        println!(
                            "{} = {}",
                            manifest.info.name.bold(),
                            format!("failed: {:#}", error).red()
                        )
                    }
                },
                List::Installed(Installed::Outdated) => {
                    match home.outdated_manifest_version(&manifest) {
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
    fn list_files(home: &Home, manifest: &Manifest, existing: bool) -> () {
        for file in home.installed_files(manifest)? {
            if !existing || file.exists() {
                println!("{}", file.display());
            }
        }
    }

    #[throws]
    fn install_manifest(home: &mut Home, name: &str, manifest: &Manifest) -> () {
        println!("Installing {}", name.bold());
        home.install_manifest(manifest)?;
        println!("{}", format!("{} installed", name).green());
    }

    #[throws]
    fn update_manifest(home: &mut Home, name: &str, manifest: &Manifest) -> () {
        if home.outdated_manifest_version(manifest)?.is_some() {
            println!("Updating {}", name.bold());
            home.remove_manifest(manifest)?;
            home.install_manifest(manifest)?;
            println!("{}", format!("{} updated", name).green());
        }
    }

    pub fn list(home: &mut Home, mode: List) -> Result<()> {
        let store = home.manifest_store()?;
        // FIXME: Don't unwrap here!  (Still we can safely assume that a store only has valid manifests to some degree)
        list_manifests(&home, store.manifests()?.map(|m| m.unwrap()), mode)
    }

    #[throws]
    pub fn files(home: &mut Home, names: Vec<String>, existing: bool) -> () {
        let store = home.manifest_store()?;
        for name in names {
            let manifest = store
                .load_manifest(&name)?
                .ok_or_else(|| anyhow!("Binary {} not found", name))?;
            list_files(home, &manifest, existing)?;
        }
    }

    #[throws]
    pub fn install(home: &mut Home, names: Vec<String>) -> () {
        let store = home.manifest_store()?;
        for name in names {
            let manifest = store
                .load_manifest(&name)?
                .ok_or_else(|| anyhow!("Binary {} not found", name))?;
            install_manifest(home, &name, &manifest)?;
        }
    }

    #[throws]
    pub fn update(home: &mut Home, names: Option<Vec<String>>) -> () {
        match names {
            None => {
                for manifest in home.manifest_store()?.manifests()? {
                    let manifest = manifest?;
                    update_manifest(home, &manifest.info.name, &manifest)?;
                }
            }
            Some(names) => {
                let store = home.manifest_store()?;
                for name in names {
                    if let Some(manifest) = store.load_manifest(&name)? {
                        update_manifest(home, &manifest.info.name, &manifest)?;
                    }
                }
            }
        }
    }

    fn read_manifests<I: Iterator<Item = R>, R: AsRef<Path>>(
        filenames: I,
    ) -> Result<Vec<Manifest>> {
        filenames.map(Manifest::read_from_path).collect()
    }

    pub fn manifest_list(home: &Home, filenames: Vec<PathBuf>, mode: List) -> Result<()> {
        list_manifests(home, read_manifests(filenames.iter())?.into_iter(), mode)
    }

    #[throws]
    pub fn manifest_files(home: &Home, filenames: Vec<PathBuf>, existing: bool) -> () {
        for manifest in read_manifests(filenames.iter())? {
            list_files(home, &manifest, existing)?
        }
    }

    #[throws]
    pub fn manifest_install(home: &mut Home, filenames: Vec<PathBuf>) -> () {
        for filename in filenames {
            let manifest = Manifest::read_from_path(&filename)?;
            install_manifest(home, &filename.display().to_string(), &manifest)?;
        }
    }

    #[throws]
    pub fn manifest_update(home: &mut Home, filenames: Vec<PathBuf>) -> () {
        for filename in filenames {
            let manifest = Manifest::read_from_path(&filename)?;
            update_manifest(home, &filename.display().to_string(), &manifest)?;
        }
    }
}

#[allow(clippy::cognitive_complexity)]
fn process_args(matches: &ArgMatches) -> anyhow::Result<()> {
    use subcommands::{Installed, List};

    let mut home = Home::open();

    match matches.subcommand() {
        ("list", _) => subcommands::list(&mut home, List::All),
        ("", _) => subcommands::list(&mut home, List::Installed(Installed::All)),
        ("installed", _) => subcommands::list(&mut home, List::Installed(Installed::All)),
        ("outdated", _) => subcommands::list(&mut home, List::Installed(Installed::Outdated)),
        ("files", Some(m)) => subcommands::files(
            &mut home,
            values_t!(m.values_of("name"), String).unwrap_or_else(|e| e.exit()),
            m.is_present("existing"),
        ),
        ("install", Some(m)) => subcommands::install(
            &mut home,
            values_t!(m.values_of("name"), String).unwrap_or_else(|e| e.exit()),
        ),
        ("update", Some(m)) => {
            let names = if m.is_present("name") {
                Some(values_t!(m.values_of("name"), String).unwrap_or_else(|e| e.exit()))
            } else {
                None
            };
            subcommands::update(&mut home, names)
        }
        ("manifest-list", Some(m)) => subcommands::manifest_list(
            &home,
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
            List::All,
        ),
        ("manifest-installed", Some(m)) => subcommands::manifest_list(
            &home,
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
            List::Installed(Installed::All),
        ),
        ("manifest-outdated", Some(m)) => subcommands::manifest_list(
            &home,
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
            List::Installed(Installed::Outdated),
        ),
        ("manifest-files", Some(m)) => subcommands::manifest_files(
            &home,
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
            m.is_present("existing"),
        ),
        ("manifest-install", Some(m)) => subcommands::manifest_install(
            &mut home,
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
        ),
        ("manifest-update", Some(m)) => subcommands::manifest_update(
            &mut home,
            values_t!(m.values_of("manifest-file"), PathBuf).unwrap_or_else(|e| e.exit()),
        ),
        (other, _) => Err(anyhow!("Unknown subcommand: {}", other)),
    }
}

fn main() {
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
