// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Command extensions

use std::io::{Error, ErrorKind, Result};
use std::process::{Command, ExitStatus, Output};

pub trait CommandExt {
    /// Spawn and wait for this command.
    fn call(&mut self) -> Result<ExitStatus>;

    /// Spawn and wait for this command and return an error if the exit code is non-zero.
    fn checked_call(&mut self) -> Result<()>;

    /// Wait for the output of this command and return an error if the exit code is non-zero.
    fn checked_output(&mut self) -> Result<Output>;
}

impl CommandExt for Command {
    fn call(&mut self) -> Result<ExitStatus> {
        self.spawn().and_then(|mut c| c.wait())
    }

    fn checked_call(&mut self) -> Result<()> {
        self.call().and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(Error::new(
                    ErrorKind::Other,
                    format!("{:?} failed with exit code {}", self, status),
                ))
            }
        })
    }

    fn checked_output(&mut self) -> Result<Output> {
        self.output().and_then(|output| {
            if output.status.success() {
                Ok(output)
            } else {
                Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "{:?} failed with exit code {}: {}",
                        self,
                        output.status,
                        String::from_utf8_lossy(&output.stderr)
                    ),
                ))
            }
        })
    }
}
