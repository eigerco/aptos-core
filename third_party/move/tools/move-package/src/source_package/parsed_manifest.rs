// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use move_core_types::account_address::AccountAddress;
use move_symbol_pool::symbol::Symbol;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, fmt::Debug, path::PathBuf};

pub type NamedAddress = Symbol;
pub type PackageName = Symbol;
pub type FileName = Symbol;

/// Per-file digest information for incremental compilation
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileDigest {
    /// SHA256 hash of the file contents
    pub hash: String,
    /// Relative path from package root
    pub path: PathBuf,
}

/// Package digest containing both overall package hash and per-file hashes
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackageDigest {
    /// Overall package hash (for backwards compatibility and quick comparison)
    pub package_hash: Symbol,
    /// Per-file hashes for incremental compilation
    pub file_digests: BTreeMap<PathBuf, String>,
}

impl PackageDigest {
    pub fn new(package_hash: Symbol, file_digests: BTreeMap<PathBuf, String>) -> Self {
        Self {
            package_hash,
            file_digests,
        }
    }

    /// Check if a specific file has changed by comparing hashes
    pub fn file_changed(&self, path: &PathBuf, new_hash: &str) -> bool {
        self.file_digests
            .get(path)
            .map(|old_hash| old_hash != new_hash)
            .unwrap_or(true) // File didn't exist before, so it's "changed"
    }

    /// Get the list of changed files between this digest and another
    pub fn get_changed_files(&self, other: &PackageDigest) -> Vec<PathBuf> {
        let mut changed = Vec::new();

        // Check all files in the new digest
        for (path, new_hash) in &other.file_digests {
            if self.file_changed(path, new_hash) {
                changed.push(path.clone());
            }
        }

        // Check for removed files
        for path in self.file_digests.keys() {
            if !other.file_digests.contains_key(path) {
                changed.push(path.clone());
            }
        }

        changed
    }
}

impl fmt::Display for PackageDigest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.package_hash)
    }
}

impl From<&str> for PackageDigest {
    fn from(hash_str: &str) -> Self {
        Self {
            package_hash: Symbol::from(hash_str),
            file_digests: BTreeMap::new(),
        }
    }
}

impl From<String> for PackageDigest {
    fn from(hash_str: String) -> Self {
        Self::from(hash_str.as_str())
    }
}

pub type AddressDeclarations = BTreeMap<NamedAddress, Option<AccountAddress>>;
pub type DevAddressDeclarations = BTreeMap<NamedAddress, AccountAddress>;
pub type Version = (u64, u64, u64);
pub type Dependencies = BTreeMap<PackageName, Dependency>;
pub type Substitution = BTreeMap<NamedAddress, SubstOrRename>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceManifest {
    pub package: PackageInfo,
    pub addresses: Option<AddressDeclarations>,
    pub dev_address_assignments: Option<DevAddressDeclarations>,
    pub build: Option<BuildInfo>,
    pub dependencies: Dependencies,
    pub dev_dependencies: Dependencies,
}

impl fmt::Display for SourceManifest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "[package]")?;
        writeln!(f, "{}", self.package)?;
        writeln!(f, "[addresses]")?;
        if let Some(address_map) = &self.addresses {
            for (named, addr_opt) in address_map.iter() {
                if let Some(addr) = addr_opt {
                    writeln!(f, "{} = \"{}\"", named.as_str(), addr)?;
                } else {
                    writeln!(f, "{} = \"_\"", named.as_str())?;
                }
            }
        }
        writeln!(f, "[dependencies]")?;
        for (package_name, dep) in self.dependencies.clone().into_iter() {
            writeln!(f, "{} = {{ local = {} }}", package_name, dep)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PackageInfo {
    pub name: PackageName,
    pub version: Version,
    pub authors: Vec<Symbol>,
    pub license: Option<Symbol>,
    pub custom_properties: BTreeMap<Symbol, String>,
}

impl fmt::Display for PackageInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "name = \"{}\"", self.name.as_str())?;
        writeln!(
            f,
            "version = \"{}.{}.{}\"",
            self.version.0, self.version.1, self.version.2
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Dependency {
    pub local: PathBuf,
    pub subst: Option<Substitution>,
    pub version: Option<Version>,
    pub digest: Option<PackageDigest>,
    pub git_info: Option<GitInfo>,
    pub node_info: Option<CustomDepInfo>,
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.local.as_os_str())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GitInfo {
    /// The git clone url to download from
    pub git_url: Symbol,
    /// The git revision, AKA, a commit SHA
    pub git_rev: Symbol,
    /// The path under this repo where the move package can be found -- e.g.,
    /// 'language/move-stdlib`
    pub subdir: PathBuf,
    /// Where the git repo is downloaded to.
    pub download_to: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CustomDepInfo {
    /// The url of the node to download from
    pub node_url: Symbol,
    /// The address where the package is published. The representation depends
    /// on the registered node resolver.
    pub package_address: Symbol,
    /// The address where the package is published.
    pub package_name: Symbol,
    /// Where the package is downloaded to.
    pub download_to: PathBuf,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct BuildInfo {
    pub language_version: Option<Version>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SubstOrRename {
    RenameFrom(NamedAddress),
    Assign(AccountAddress),
}
