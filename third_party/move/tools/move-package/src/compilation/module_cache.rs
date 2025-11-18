// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

//! Content-addressed module cache for incremental compilation.
//!
//! This module provides a persistent cache for compiled Move modules, indexed by the hash
//! of their source code and compilation flags. This enables massive speedups for mutation
//! testing and other scenarios where only a subset of modules change between compilations.

use anyhow::{Context, Result};
use move_binary_format::file_format::CompiledModule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Key for looking up cached modules
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheKey {
    /// SHA256 hash of the source file
    pub file_hash: String,
    /// Whether test mode was enabled during compilation
    pub test_mode: bool,
    /// Whether dev mode was enabled
    pub dev_mode: bool,
}

impl CacheKey {
    pub fn new(file_hash: String, test_mode: bool, dev_mode: bool) -> Self {
        Self {
            file_hash,
            test_mode,
            dev_mode,
        }
    }

    /// Generate a filesystem-safe cache filename
    fn cache_filename(&self) -> String {
        format!(
            "{}_test{}_dev{}.bin",
            self.file_hash,
            if self.test_mode { "1" } else { "0" },
            if self.dev_mode { "1" } else { "0" }
        )
    }
}

/// A cached compiled module with its interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedModule {
    /// The compiled bytecode (serialized)
    pub bytecode_bytes: Vec<u8>,
    /// Source file path (for debugging/logging)
    pub source_path: PathBuf,
    /// When this was cached
    pub cache_timestamp: u64,
}

impl CachedModule {
    /// Create a new cached module from a CompiledModule
    pub fn new(module: &CompiledModule, source_path: PathBuf) -> Result<Self> {
        let mut bytecode_bytes = Vec::new();
        module.serialize(&mut bytecode_bytes)
            .context("Failed to serialize compiled module")?;

        Ok(Self {
            bytecode_bytes,
            source_path,
            cache_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Deserialize the bytecode back to a CompiledModule
    pub fn to_compiled_module(&self) -> Result<CompiledModule> {
        CompiledModule::deserialize(&self.bytecode_bytes)
            .context("Failed to deserialize compiled module")
    }
}

/// Content-addressed cache for compiled modules
pub struct ModuleCache {
    cache_dir: PathBuf,
    /// In-memory cache for this session
    memory_cache: HashMap<CacheKey, CachedModule>,
}

impl ModuleCache {
    /// Create a new module cache using the default cache directory
    pub fn new() -> Result<Self> {
        let cache_dir = Self::default_cache_dir()?;
        Self::with_cache_dir(cache_dir)
    }

    /// Create a module cache with a specific cache directory
    pub fn with_cache_dir(cache_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create cache directory: {:?}", cache_dir))?;

        Ok(Self {
            cache_dir,
            memory_cache: HashMap::new(),
        })
    }

    /// Get the default cache directory (~/.move/module_cache/)
    fn default_cache_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to get home directory")?;
        Ok(home.join(".move").join("module_cache"))
    }

    /// Get a cached module if it exists
    pub fn get(&mut self, key: &CacheKey) -> Option<CachedModule> {
        // Check memory cache first
        if let Some(cached) = self.memory_cache.get(key) {
            return Some(cached.clone());
        }

        // Check disk cache
        let cache_path = self.cache_path(key);
        if !cache_path.exists() {
            return None;
        }

        // Try to load from disk
        let bytes = fs::read(&cache_path).ok()?;
        let cached: CachedModule = bcs::from_bytes(&bytes).ok()?;

        // Store in memory cache for this session
        self.memory_cache.insert(key.clone(), cached.clone());

        Some(cached)
    }

    /// Insert a module into the cache
    pub fn insert(&mut self, key: CacheKey, module: CachedModule) -> Result<()> {
        // Store in memory cache
        self.memory_cache.insert(key.clone(), module.clone());

        // Persist to disk (atomic write: tmp + rename)
        let cache_path = self.cache_path(&key);
        let tmp_path = cache_path.with_extension("tmp");

        let bytes = bcs::to_bytes(&module)
            .context("Failed to serialize cached module")?;

        fs::write(&tmp_path, bytes)
            .with_context(|| format!("Failed to write cache file: {:?}", tmp_path))?;

        fs::rename(&tmp_path, &cache_path)
            .with_context(|| format!("Failed to rename cache file: {:?}", cache_path))?;

        Ok(())
    }

    /// Get the filesystem path for a cache key
    fn cache_path(&self, key: &CacheKey) -> PathBuf {
        self.cache_dir.join(key.cache_filename())
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let disk_entries = fs::read_dir(&self.cache_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0);

        CacheStats {
            memory_entries: self.memory_cache.len(),
            disk_entries,
            cache_dir: self.cache_dir.clone(),
        }
    }

    /// Clear the entire cache (both memory and disk)
    pub fn clear(&mut self) -> Result<()> {
        self.memory_cache.clear();

        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)
                .with_context(|| format!("Failed to clear cache directory: {:?}", self.cache_dir))?;
            fs::create_dir_all(&self.cache_dir)?;
        }

        Ok(())
    }
}

impl Default for ModuleCache {
    fn default() -> Self {
        Self::new().expect("Failed to create default module cache")
    }
}

/// Statistics about the module cache
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub memory_entries: usize,
    pub disk_entries: usize,
    pub cache_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use move_binary_format::file_format::*;
    use move_core_types::identifier::Identifier;

    fn create_test_module() -> CompiledModule {
        use move_core_types::account_address::AccountAddress;

        CompiledModule {
            version: 7,
            self_module_handle_idx: ModuleHandleIndex(0),
            module_handles: vec![ModuleHandle {
                address: AddressIdentifierIndex(0),
                name: IdentifierIndex(0),
            }],
            struct_handles: vec![],
            function_handles: vec![],
            field_handles: vec![],
            friend_decls: vec![],
            struct_defs: vec![],
            struct_def_instantiations: vec![],
            struct_variant_handles: vec![],
            struct_variant_instantiations: vec![],
            variant_field_handles: vec![],
            variant_field_instantiations: vec![],
            function_defs: vec![],
            function_instantiations: vec![],
            field_instantiations: vec![],
            signatures: vec![],
            identifiers: vec![Identifier::new("TestModule").unwrap()],
            address_identifiers: vec![AccountAddress::ZERO],
            constant_pool: vec![],
            metadata: vec![],
        }
    }

    #[test]
    fn test_cache_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut cache = ModuleCache::with_cache_dir(temp_dir.path().to_path_buf()).unwrap();

        let key = CacheKey::new("abcd1234".to_string(), true, true);
        let module = create_test_module();
        let cached = CachedModule::new(&module, PathBuf::from("test.move")).unwrap();

        // Insert
        cache.insert(key.clone(), cached.clone()).unwrap();

        // Retrieve
        let retrieved = cache.get(&key).unwrap();
        assert_eq!(retrieved.source_path, cached.source_path);

        // Verify bytecode round-trips correctly
        let retrieved_module = retrieved.to_compiled_module().unwrap();
        // Version may be upgraded during serialization, just check module name
        assert_eq!(
            retrieved_module.self_id().name(),
            module.self_id().name()
        );
    }

    #[test]
    fn test_cache_miss() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut cache = ModuleCache::with_cache_dir(temp_dir.path().to_path_buf()).unwrap();

        let key = CacheKey::new("nonexistent".to_string(), false, false);
        assert!(cache.get(&key).is_none());
    }
}
