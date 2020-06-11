// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A command line tool to install binaries to $HOME.

#![deny(warnings, clippy::all, missing_docs)]

use clap::*;
use colored::*;
use fehler::{throw, throws};

use anyhow::{anyhow, Error};
use homebins::{Home, ManifestStore};

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

#[throws]
fn list(store: &ManifestStore, home: &Home, mode: List) -> () {
    let mut failed = false;
    for manifest_res in store.manifests()? {
        let manifest = manifest_res?;

        match mode {
            List::All => println!("{}: {}", manifest.info.name, manifest.info.version),
            List::Installed(mode) => match (home.installed_manifest_version(&manifest), mode) {
                (Ok(Some(version)), Installed::All) => {
                    println!("{} = {}", manifest.info.name, version)
                }
                (Ok(Some(version)), Installed::Outdated) if version < manifest.info.version => {
                    println!(
                        "{} = {} -> {}",
                        manifest.info.name, version, manifest.info.version
                    )
                }
                (Err(error), _) => {
                    eprintln!(
                        "{}",
                        format!(
                            "Failed to check version of {}: {:#}",
                            manifest.info.name, error
                        )
                        .red()
                        .bold()
                    );
                    failed = true;
                }
                _ => {}
            },
        }
    }
    if failed {
        throw!(anyhow!("Some version checks failed"));
    }
}

#[throws]
fn files(store: &ManifestStore, home: &Home, existing: bool, names: Vec<String>) -> () {
    for name in names {
        let manifest = store
            .load_manifest(&name)?
            .ok_or_else(|| anyhow!("Binary {} not found", name))?;
        for install in manifest.install {
            for file in install.files {
                let target = home.target(&file)?;
                if !existing || target.exists() {
                    println!("{}", target.display());
                }
            }
        }
    }
}

#[throws]
fn install(home: &mut Home, store: &ManifestStore, names: Vec<String>) -> () {
    for name in names {
        let manifest = store
            .load_manifest(&name)?
            .ok_or_else(|| anyhow!("Binary {} not found", name))?;
        println!("Installing {}", name.bold());
        home.install_manifest(&manifest)?;
        println!("{}", format!("{} installed", name).green());
    }
}

fn process_args(matches: &ArgMatches) -> anyhow::Result<()> {
    let mut home = Home::open();

    let repo = home.cloned_manifest_repo(
        "https://github.com/lunaryorn/homebin-manifests".into(),
        "lunaryorn",
    )?;

    let store = repo.store();
    match matches.subcommand() {
        ("list", _) => list(&store, &home, List::All),
        ("installed", _) => list(&store, &home, List::Installed(Installed::All)),
        ("outdated", _) => list(&store, &home, List::Installed(Installed::Outdated)),
        ("files", Some(m)) => {
            let names = values_t!(m.values_of("name"), String).unwrap_or_else(|e| e.exit());
            files(&store, &home, m.is_present("existing"), names)
        }
        ("install", Some(m)) => {
            let names = values_t!(m.values_of("name"), String).unwrap_or_else(|e| e.exit());
            install(&mut home, &store, names)
        }
        (other, _) => Err(anyhow!("Unknown subcommand: {}", other)),
    }
}

fn main() {
    let app = app_from_crate!()
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::ColoredHelp)
        .subcommand(SubCommand::with_name("list").about("List available binaries"))
        .subcommand(SubCommand::with_name("installed").about("List installed binaries"))
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
            SubCommand::with_name("install").about("Install bins").arg(
                Arg::with_name("name")
                    .required(true)
                    .multiple(true)
                    .help("Binaries to install"),
            ),
        );

    if let Err(error) = process_args(&app.get_matches()) {
        eprintln!("{}", format!("Error: {:#}", error).red().bold());
        std::process::exit(1)
    }
}
