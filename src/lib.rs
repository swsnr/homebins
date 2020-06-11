// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Install binaries to $HOME.
//!
//! Not a package manager.

#![deny(warnings, clippy::all, missing_docs)]

mod home;
mod manifest;

pub use home::Home;
pub use manifest::repo::ManifestRepo;
pub use manifest::store::ManifestStore;
pub use manifest::types::*;
