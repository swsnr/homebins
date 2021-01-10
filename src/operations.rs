// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

pub use apply::ApplyOperation;
pub use install::install_manifest;
pub use remove::remove_manifest;
pub use types::*;
pub use update::update_manifest;
pub use util::operation_destinations;

mod apply;
mod install;
mod remove;
mod types;
mod update;
mod util;
