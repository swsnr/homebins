// Copyright 2020 Sebastian Wiesner <sebastian@swsnr.de>

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Checksum validation.

use crate::manifest::Checksums;
use digest::Digest;
use std::io::{Read, Write};
use thiserror::Error;

/// A checksum validation error.
#[derive(Error, Debug)]
pub enum ValidationError {
    /// The checksum didn't match.
    #[error("The checksum didn't match, got {actual}")]
    ChecksumMismatch { actual: String },
    /// The checksum was empty.
    #[error("The checksum was empty")]
    ChecksumEmpty,
    /// An IO error occurred while reading data to validate.
    #[error("Reading failed: {0}")]
    IO(#[from] std::io::Error),
}

pub trait Validate {
    /// Validate the data read from the given source.
    fn validate<R: Read>(&self, source: &mut R) -> Result<(), ValidationError>;
}

fn validate<D: Digest + Write, R: Read>(
    reader: &mut R,
    checksum: &[u8],
) -> Result<(), ValidationError> {
    if checksum.is_empty() {
        Err(ValidationError::ChecksumEmpty)
    } else {
        let mut digest = D::new();
        std::io::copy(reader, &mut digest)?;
        let hash = digest.finalize();
        if hash.as_slice() == checksum {
            Ok(())
        } else {
            Err(ValidationError::ChecksumMismatch {
                actual: hex::encode(hash),
            })
        }
    }
}

impl Validate for Checksums {
    fn validate<R: Read>(&self, source: &mut R) -> Result<(), ValidationError> {
        match self {
            Checksums { b2: Some(b2), .. } => validate::<blake2::Blake2b, _>(source, &b2),
            Checksums {
                sha512: Some(sha512),
                ..
            } => validate::<sha2::Sha512, _>(source, &sha512),
            Checksums {
                sha256: Some(sha256),
                ..
            } => validate::<sha2::Sha256, _>(source, &sha256),
            Checksums {
                sha1: Some(sha1), ..
            } => validate::<sha1::Sha1, _>(source, &sha1),
            Checksums { sha1: None, .. } => Err(ValidationError::ChecksumEmpty),
        }
    }
}
