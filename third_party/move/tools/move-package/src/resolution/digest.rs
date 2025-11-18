// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::source_package::{layout::SourcePackageLayout, parsed_manifest::PackageDigest};
use anyhow::Result;
use move_command_line_common::files::MOVE_EXTENSION;
use move_symbol_pool::Symbol;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn compute_digest(paths: &[PathBuf]) -> Result<PackageDigest> {
    let mut file_digests = BTreeMap::new();
    let mut sorted_hashes = Vec::new();

    let mut hash_file = |path: &Path| -> Result<()> {
        let contents = std::fs::read(path)?;
        let hash = format!("{:X}", Sha256::digest(&contents));
        file_digests.insert(path.to_path_buf(), hash.clone());
        sorted_hashes.push((path.to_path_buf(), hash));
        Ok(())
    };

    let mut maybe_hash_file = |path: &Path| -> Result<()> {
        match path.extension() {
            Some(x) if MOVE_EXTENSION == x => hash_file(path),
            _ if path.ends_with(SourcePackageLayout::Manifest.path()) => hash_file(path),
            _ => Ok(()),
        }
    };

    for path in paths {
        if path.is_file() {
            maybe_hash_file(path)?;
        } else {
            for entry in walkdir::WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    maybe_hash_file(entry.path())?
                }
            }
        }
    }

    // Sort by path to ensure stable ordering
    sorted_hashes.sort_by(|a, b| a.0.cmp(&b.0));

    // Compute overall package hash from sorted file hashes
    let mut hasher = Sha256::new();
    for (_, file_hash) in sorted_hashes.into_iter() {
        hasher.update(file_hash.as_bytes());
    }

    let package_hash = Symbol::from(format!("{:X}", hasher.finalize()));

    Ok(PackageDigest::new(package_hash, file_digests))
}
