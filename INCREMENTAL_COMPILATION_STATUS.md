# Move Compiler Incremental Compilation - Implementation Status

## Summary

This document describes the incremental compilation infrastructure added to the Move compiler to support faster mutation testing and iterative development.

## What Was Implemented

### ✅ Phase 1: Per-File Hash Tracking

**Files Modified:**
- `third_party/move/tools/move-package/src/source_package/parsed_manifest.rs`
- `third_party/move/tools/move-package/src/resolution/digest.rs`

**Changes:**
1. Changed `PackageDigest` from a simple type alias (`Symbol`) to a struct containing:
   - `package_hash`: Overall package hash (backwards compatible)
   - `file_digests`: `BTreeMap<PathBuf, String>` mapping each file to its SHA256 hash

2. Updated `compute_digest()` to:
   - Hash each `.move` file individually
   - Store per-file hashes alongside the package hash
   - Maintain stable ordering for reproducible builds

3. Added helper methods:
   - `PackageDigest::get_changed_files()` - returns list of files that changed between two digests
   - `PackageDigest::file_changed()` - checks if a specific file changed

**Benefit:** Can now track exactly which files changed between compilations.

### ✅ Phase 2: Content-Addressed Module Cache

**Files Created:**
- `third_party/move/tools/move-package/src/compilation/module_cache.rs`

**Files Modified:**
- `third_party/move/tools/move-package/src/compilation/mod.rs`
- `third_party/move/tools/move-package/Cargo.toml` (added `bcs` and `dirs` dependencies)

**Implementation:**
```rust
pub struct ModuleCache {
    cache_dir: PathBuf,  // ~/.move/module_cache/
    memory_cache: HashMap<CacheKey, CachedModule>,
}

pub struct CacheKey {
    file_hash: String,      // SHA256 of source file
    test_mode: bool,        // Compilation context
    dev_mode: bool,
}

pub struct CachedModule {
    bytecode_bytes: Vec<u8>,  // Serialized CompiledModule
    source_path: PathBuf,
    cache_timestamp: u64,
}
```

**Features:**
- **Content-addressed:** Modules keyed by source hash + compilation flags
- **Two-level caching:** In-memory + persistent disk cache
- **Parallel-safe:** Multiple processes can read cache simultaneously
- **Atomic writes:** Uses temp file + rename to prevent corruption
- **Location:** `~/.move/module_cache/`

**API:**
```rust
let mut cache = ModuleCache::new()?;
let key = CacheKey::new(file_hash, test_mode, dev_mode);

// Cache compiled module
let cached = CachedModule::new(&compiled_module, source_path)?;
cache.insert(key, cached)?;

// Retrieve from cache
if let Some(cached) = cache.get(&key) {
    let module = cached.to_compiled_module()?;
}
```

**Tests:** Comprehensive unit tests in `module_cache::tests`

### ✅ Phase 3: File Change Tracking Integration

**Files Modified:**
- `third_party/move/tools/move-package/src/compilation/compiled_package.rs`

**Changes:**
1. Added `OnDiskCompiledPackage::get_changed_files()` method
   - Returns list of files that changed since last compilation
   - Uses the per-file hashes from `PackageDigest`

2. Added verbose compilation logging:
   - Set `MOVE_VM_VERBOSE_COMPILATION=1` to see which files changed
   - Shows per-file change tracking in action
   - Identifies the test_mode forced recompilation bottleneck

**Example output:**
```bash
$ MOVE_VM_VERBOSE_COMPILATION=1 aptos move test
[incremental] Package 'my_package': 1 file(s) changed:
[incremental]   - sources/module_a.move
[incremental] Package 'my_package': No files changed, but forced recompilation due to test_mode
```

## Current Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Resolution & Digest Computation                            │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  For each .move file:                                │   │
│  │    - Read file content                               │   │
│  │    - Compute SHA256 hash                             │   │
│  │    - Store in file_digests map                       │   │
│  │  Compute package_hash from all file hashes          │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Compilation Decision                                        │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  can_load_cached():                                  │   │
│  │    - Compare package digests                         │   │
│  │    - Check test_mode flag                            │   │
│  │    - Check force_recompilation flag                  │   │
│  │                                                       │   │
│  │  If cached: Skip compilation ✓                       │   │
│  │  If not: Compile all files in package ✗             │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Module Cache (Ready for Future Use)                         │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  ~/.move/module_cache/                               │   │
│  │    {file_hash}_test1_dev1.bin                       │   │
│  │    {file_hash}_test0_dev0.bin                       │   │
│  │                                                       │   │
│  │  Not yet integrated with compilation pipeline        │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Current State for Mutation Testing

### What Works Now:
1. **Per-file change detection** - Can identify exactly which file changed
2. **Module cache infrastructure** - Ready to store/retrieve compiled modules
3. **Dependency caching** - Dependencies (aptos-framework, etc.) are not recompiled
4. **Visibility** - Can see what's happening with `MOVE_VM_VERBOSE_COMPILATION=1`

### What Doesn't Work Yet:
1. **Per-file incremental compilation within a package** - When one file changes, all 100 files still recompile
2. **test_mode bottleneck** - Line 554 in `compiled_package.rs` forces recompilation of root package in test mode

### Why It's Hard:
The Move compiler's type-checking phase requires a **global environment** with all modules:
- Parses all files together
- Type-checks all modules together (cross-module references, generics, etc.)
- Cannot easily type-check one module in isolation

To achieve true per-file incremental compilation, we would need:
1. **Cached ASTs** for unchanged files
2. **Interface extraction** from cached bytecode (function signatures, struct definitions)
3. **Incremental type-checking** that uses cached interfaces for unchanged modules
4. **Selective bytecode generation** for only changed modules

**Estimated effort:** 2-3 weeks of deep compiler refactoring

## Recommended Next Steps

### Option A: Quick Win (1-2 days)
**Remove test_mode forced recompilation for mutation testing**

Modify `compiled_package.rs:554`:
```rust
// Before:
|| resolution_graph.build_options.test_mode && is_root_package

// After:
|| (resolution_graph.build_options.test_mode
    && is_root_package
    && !resolution_graph.build_options.incremental_test_mode)  // New flag
```

Add `--incremental` flag to `aptos move test` that:
- Sets `incremental_test_mode = true`
- Allows package-level caching even in test mode
- Still recompiles if any file changed

**Benefit:** Avoids recompilation when testing unchanged code (useful for running same tests multiple times)

**Limitation:** Still recompiles all 100 files when 1 file changes (no per-file incremental compilation)

### Option B: Full Per-File Incremental Compilation (2-3 weeks)

Implement true per-file incremental compilation:

1. **Week 1: Cached AST & Interface Extraction**
   - Store parsed ASTs for each file
   - Extract module interfaces (function/struct signatures) from bytecode
   - Build partial GlobalEnv with cached interfaces

2. **Week 2: Incremental Type Checking**
   - Modify type checker to accept pre-populated GlobalEnv
   - Type-check only changed modules against cached interfaces
   - Handle cross-module dependencies correctly

3. **Week 3: Integration & Testing**
   - Wire everything together in compilation pipeline
   - Test with mutation testing workload
   - Measure speedup (expected: ~90x for 100-file project)

### Option C: Alternative Approach - Bytecode Mutation

Instead of faster compilation, mutate the **bytecode directly**:
- Compile once to bytecode
- For each mutant: modify bytecode (change opcodes/constants)
- Run tests on mutated bytecode
- No recompilation needed

**Pros:** No compiler changes needed, maximum speed
**Cons:** Requires building a bytecode mutation tool, limited mutation types

## Usage for Mutation Testing

### Current Best Practice:

1. **Use the infrastructure we built:**
```rust
// In your mutation testing tool:
use move_package::compilation::module_cache::{ModuleCache, CacheKey};

let cache = ModuleCache::new()?;

// After compiling original code:
for (file_path, compiled_module) in compiled_modules {
    let file_hash = compute_file_hash(&file_path)?;
    let key = CacheKey::new(file_hash, true, true);
    let cached = CachedModule::new(&compiled_module, file_path)?;
    cache.insert(key, cached)?;
}

// For each mutant:
// 1. Detect which file changed (using PackageDigest::get_changed_files)
// 2. Check cache for unchanged modules
// 3. Only recompile if cache miss or force compilation
```

2. **Enable verbose logging:**
```bash
MOVE_VM_VERBOSE_COMPILATION=1 move-mutation-test --your-args
```

This shows which files changed for each mutant, validating the tracking works.

### Expected Performance:

**Without per-file incremental compilation (current state):**
- 100 files, 1000 mutants = 100,000 file compilations
- Dependencies cached ✓
- Within-package files: not cached ✗

**With per-file incremental compilation (after Option B):**
- 100 files, 1000 mutants = ~1,100 file compilations
- **~90x speedup**
- Hours → Minutes

## Testing the Implementation

```bash
# Run module cache tests
cargo test -p move-package --lib compilation::module_cache

# Test with a real Move project
cd your-move-project
MOVE_VM_VERBOSE_COMPILATION=1 aptos move test

# You should see output like:
# [incremental] Package 'your_package': 1 file(s) changed:
# [incremental]   - sources/changed_file.move
```

## Code Locations

| Component | File | Key Functions/Structs |
|-----------|------|----------------------|
| Per-file hashing | `source_package/parsed_manifest.rs` | `PackageDigest`, `get_changed_files()` |
| Digest computation | `resolution/digest.rs` | `compute_digest()` |
| Module cache | `compilation/module_cache.rs` | `ModuleCache`, `CacheKey`, `CachedModule` |
| Cache integration | `compilation/compiled_package.rs` | `can_load_cached()`, `get_changed_files()` |
| Test mode forcing | `compilation/compiled_package.rs:554` | **Main bottleneck for mutation testing** |

## Contact & Questions

This implementation provides the foundation for incremental compilation. The infrastructure is solid and tested. The remaining work is integrating it into the compilation pipeline to actually skip recompiling unchanged files.

For questions or to continue this work, the key challenges are:
1. Building a GlobalEnv with mix of cached and new modules
2. Incremental type-checking
3. Handling cross-module dependencies during partial compilation

Good luck! The ~90x speedup for mutation testing is within reach. 🚀
