// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use anyhow::{anyhow, Error};
use clap::*;
use fehler::{throw, throws};
use homebins::{Home, ManifestStore};
use std::path::Path;

#[throws]
fn list_installed(home: &Home, store: &ManifestStore) -> () {
    let mut failed = false;
    for manifest_res in store.manifests()? {
        let manifest = manifest_res?;
        match home.installed_manifest_version(&manifest) {
            Ok(None) => {}
            Ok(Some(version)) => {
                println!("{} -> {}", manifest.meta.name, version);
            }
            Err(error) => {
                eprintln!(
                    "Failed to check version of {}: {:#}",
                    manifest.meta.name, error
                );
                failed = true;
            }
        }
    }

    if failed {
        throw!(anyhow!("Some version checks failed"));
    }
}

#[throws]
fn install(home: &mut Home, store: &ManifestStore, names: Vec<String>) -> () {
    for name in names {
        let manifest = store
            .load_manifest(&name)?
            .ok_or(anyhow!("Binary {} not found", name))?;
        home.install_manifest(&manifest)?;
    }
}

fn process_args(matches: &ArgMatches) -> anyhow::Result<()> {
    let mut home = Home::new()?;
    let store = ManifestStore::open(Path::new("manifests/").to_path_buf());
    match matches.subcommand() {
        ("list", _) => list_installed(&home, &store),
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
        .subcommand(SubCommand::with_name("list").about("List installed bins"))
        .subcommand(
            SubCommand::with_name("install").about("Install bins").arg(
                Arg::with_name("name")
                    .required(true)
                    .multiple(true)
                    .help("Binaries to install"),
            ),
        );

    if let Err(error) = process_args(&app.get_matches()) {
        eprintln!("Error: {:#}", error);
        std::process::exit(1)
    }
}
