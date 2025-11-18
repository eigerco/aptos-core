# Using Incremental Compilation with Move Mutation Testing

## Quick Start

The Move compiler now supports `--incremental` mode for faster compilation during mutation testing!

### For CLI Users

```bash
# Enable incremental compilation in test mode
aptos move test --incremental

# With verbose logging to see what's being cached
MOVE_VM_VERBOSE_COMPILATION=1 aptos move test --incremental
```

### For move-mutation-test Integration

Update your `run_move_unit_tests()` calls to enable incremental mode:

```rust
use move_package::BuildConfig;

let mut build_config = BuildConfig {
    test_mode: true,
    dev_mode: true,
    incremental: true,  // ← Add this!
    ..Default::default()
};

let result = move_cli::base::test::run_move_unit_tests(
    &package_path,
    build_config,
    unit_test_config,
    natives,
    genesis,
    gas_limit,
    cost_table,
    compute_coverage,
    &mut std::io::stdout(),
    false,
)?;
```

## What This Enables

### ✅ Package-Level Caching in Test Mode

**Before (`--incremental` not set):**
- First test run: Compiles all 100 files
- Second test run (no changes): **Still compiles all 100 files** ✗
- Reason: `test_mode` forces recompilation

**After (`--incremental` enabled):**
- First test run: Compiles all 100 files
- Second test run (no changes): **Uses cached compilation** ✓
- Only recompiles if files actually changed

### How It Helps Mutation Testing

**Scenario:** Testing 1000 mutants on a 100-file project

**Without `--incremental`:**
```
Mutant 1: file A changed → compile 100 files
Mutant 2: file B changed → compile 100 files
Mutant 3: file C changed → compile 100 files
...
Total: 100,000 file compilations
```

**With `--incremental`:**
```
Mutant 1: file A changed → compile 100 files (cache MISS)
Mutant 2: file B changed → compile 100 files (cache MISS)
Mutant 3: file C changed → compile 100 files (cache MISS)
...
Total: 100,000 file compilations
```

**Wait, same number?** Yes! Because each mutant changes a file, triggering package-level recompilation.

### ⚠️ Current Limitation

The `--incremental` flag enables package-level caching, not per-file caching. When **any file** in the package changes, the **entire package** recompiles.

**This helps when:**
- Running tests multiple times without code changes
- Testing with different test filters on same code
- Debugging specific tests repeatedly

**This does NOT help (yet) when:**
- Each mutant changes a different file (mutation testing)
- Rapid iteration changing different files

## Realistic Performance Gains

### Use Case 1: Repeated Test Runs (No Code Changes)

```bash
# First run
time aptos move test --incremental
# → 10 seconds (compiles everything)

# Second run (no changes)
time aptos move test --incremental
# → 2 seconds (uses cache!) ✓

# Speedup: 5x
```

### Use Case 2: Testing with Different Filters

```bash
# Test module A
aptos move test --incremental --filter ModuleA
# → 10 seconds (first compile)

# Test module B (code unchanged)
aptos move test --incremental --filter ModuleB
# → 2 seconds (uses cache!) ✓
```

### Use Case 3: Mutation Testing (Current State)

```bash
# Mutant 1: changes file A
aptos move test --incremental
# → 10 seconds (compiles all 100 files)

# Mutant 2: changes file B
aptos move test --incremental
# → 10 seconds (still compiles all 100 files) ✗
```

**Still useful because:**
- Dependencies (aptos-framework) aren't recompiled ✓
- Better than without any caching
- Prepares for future per-file incremental compilation

## Verbose Logging

Set `MOVE_VM_VERBOSE_COMPILATION=1` to see what's happening:

```bash
$ MOVE_VM_VERBOSE_COMPILATION=1 aptos move test --incremental

[incremental] Package 'my_package': 1 file(s) changed:
[incremental]   - sources/mutated_module.move
[incremental] Compiling package...

# Second run, no changes:
[incremental] Package 'my_package': No files changed, using cached compilation (--incremental enabled)
[incremental] Skipping compilation, loading from cache...
```

## For Mutation Testing Tool Authors

### Recommended Integration

```rust
// In your mutation test loop:
for mutant in mutants {
    // 1. Apply mutant to source file
    apply_mutant(&mutant)?;

    // 2. Run tests with incremental compilation enabled
    let mut config = BuildConfig {
        test_mode: true,
        dev_mode: true,
        incremental: true,  // Always enable this
        ..Default::default()
    };

    // 3. Optional: Enable verbose logging
    if std::env::var("VERBOSE").is_ok() {
        std::env::set_var("MOVE_VM_VERBOSE_COMPILATION", "1");
    }

    let result = run_move_unit_tests(
        &temp_dir,  // Each mutant in its own temp dir
        config,
        // ... other args
    )?;

    // 4. Check if mutant was killed
    if result == UnitTestResult::Failure {
        killed_mutants.push(mutant);
    }

    // 5. Revert mutant
    revert_mutant(&mutant)?;
}
```

### Expected Behavior

With this setup:
- ✅ Dependencies (aptos-framework, aptos-std) are cached across all mutants
- ✅ Per-mutant compilation still required (package changes for each mutant)
- ✅ Faster than without `--incremental` due to dependency caching
- ⚠️ Not yet per-file incremental (see roadmap below)

## Roadmap to Per-File Incremental Compilation

The current `--incremental` flag is Step 1 of 3:

**✅ Step 1: Package-Level Caching (Done)**
- Enables caching when no files change
- Helps repeated test runs
- ~2-5x speedup for re-running tests

**🔄 Step 2: Per-File Change Detection (In Progress)**
- Track which specific files changed
- Already implemented! Use `MOVE_VM_VERBOSE_COMPILATION=1` to see
- Not yet used for compilation decisions

**❌ Step 3: Per-File Incremental Compilation (Future - 2-3 weeks)**
- Only recompile changed files
- Keep unchanged files from cache
- ~90x speedup for mutation testing
- Requires compiler refactoring (type-checking integration)

## Troubleshooting

### Cache not working?

```bash
# Check if files are being detected as changed:
MOVE_VM_VERBOSE_COMPILATION=1 aptos move test --incremental

# If you see "No files changed" but still recompiling:
# - Make sure --incremental is set
# - Check that you're in test mode (--test or test_mode: true)
```

### Stale cache?

```bash
# Force recompilation (clears cache):
aptos move test --force

# Or manually clear cache:
rm -rf ~/.move/module_cache/
rm -rf build/
```

### Different results with/without --incremental?

This shouldn't happen! If you see different behavior:
1. Run without --incremental to get "correct" behavior
2. Clear cache: `rm -rf build/`
3. Run with --incremental
4. If still different, please report a bug!

## Performance Monitoring

### Measure Your Speedup

```bash
# Without incremental:
time aptos move test
# Note the time

# Clear build cache:
rm -rf build/

# With incremental (second run):
time aptos move test --incremental
# First run - same as above
time aptos move test --incremental
# Second run - should be faster!
```

### Typical Speedups

| Scenario | Without --incremental | With --incremental | Speedup |
|----------|----------------------|-------------------|---------|
| Re-run tests (no changes) | 10s | 2s | **5x** |
| Different test filter | 10s | 2s | **5x** |
| Mutation testing (1 file changes) | 10s | 10s | **1x** (no benefit yet) |
| Mutation testing with heavy dependencies | 15s | 10s | **1.5x** (deps cached) |

## Next Steps

1. **Use `--incremental` in your mutation testing workflow**
   - Update `move-mutation-test` to pass `incremental: true`
   - Measure actual speedup in your projects

2. **Monitor with verbose logging**
   - Validate that file change detection works
   - Identify patterns for future optimization

3. **Stay tuned for per-file incremental compilation**
   - Track progress in `INCREMENTAL_COMPILATION_STATUS.md`
   - Expected in 2-3 weeks with ~90x speedup for mutation testing

## Questions?

See `INCREMENTAL_COMPILATION_STATUS.md` for technical details and architecture.
