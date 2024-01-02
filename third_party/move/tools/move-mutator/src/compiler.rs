use move_command_line_common::address::NumericalAddress;
use move_command_line_common::parser::NumberFormat;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::configuration::Configuration;
use move_compiler::diagnostics::FilesSourceText;
use move_compiler::{
    command_line::compiler::*, diagnostics::unwrap_or_report_diagnostics, shared::Flags,
};
use move_package::source_package::layout::SourcePackageLayout;
use move_package::BuildConfig;

/// Generate the AST from the Move sources.
///
/// Generation of the AST is done by the Move compiler. Move compiler is stepped compiler, which means that
/// it is possible to get the intermediate results of the compilation. This function uses it to get the AST
/// right after the parsing phase.
///
/// Generated AST contains all the information for all the Move files provided in the `source_files` vector.
/// Compiler searches automatically for all the needed files (like manifest) and dependencies. In case of
/// any error, that error is returned.
///
/// # Arguments
///
/// * `source_files` - vector of strings representing the Move source files paths.
/// * `config` - contains the actual build configuration.
/// * `package_path` - the path to the Move package.
///
/// # Returns
///
/// * `Result<(FilesSourceText, move_compiler::parser::ast::Program), anyhow::Error>` - tuple of FilesSourceText and Program if successful, or an error if any error occurs.
pub fn generate_ast(
    mutator_config: &Configuration,
    config: &BuildConfig,
    package_path: PathBuf,
) -> Result<(FilesSourceText, move_compiler::parser::ast::Program), anyhow::Error> {
    let source_files = mutator_config
        .project
        .move_sources
        .iter()
        .map(|p| p.to_str().unwrap_or(""))
        .collect::<Vec<_>>();

    let named_addr_map = config
        .additional_named_addresses
        .clone()
        .into_iter()
        .map(|(name, addr)| {
            (
                name,
                NumericalAddress::new(addr.into_bytes(), NumberFormat::Decimal),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let out_dir = "mutator_build";
    let interface_files_dir = format!("{}/generated_interface_files", out_dir);
    let flags = Flags::empty();

    //TODO(asmie): check if root is not found (we cannot parse then Move.toml and get the address resolution)
    // Maybe we should then allow only source files with numerical addresses
    let _rooted_path = SourcePackageLayout::try_find_root(&package_path.canonicalize()?);

    let (files, res) = Compiler::from_files(
        source_files,
        vec![],
        named_addr_map,
        flags,
        &config.compiler_config.known_attributes,
    )
    .set_interface_files_dir(interface_files_dir)
    .run::<PASS_PARSER>()?;

    let (_, stepped) = unwrap_or_report_diagnostics(&files, res);
    let (_, ast) = stepped.into_ast();

    Ok((files, ast))
}

/// Verify the mutant.
/// This function compiles the mutated source and checks if the compilation is successful.
/// If the compilation is successful, the mutant is valid.
///
/// This function uses the Move compiler to compile the mutated source. To do so, it copies the whole package
/// to a temporary directory and replaces the original file with the mutated source. It may introduce problems
/// with dependencies that are specified as relative paths to the package root.
///
/// # Arguments
///
/// * `mutator_config` - the configuration for the mutator.
/// * `config` - the build configuration.
/// * `mutated_source` - the mutated source code as a string.
/// * `original_file` - the path to the original file.
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Ok if the mutant is valid, or an error if any error occurs.
pub fn verify_mutant(
    mutator_config: &Configuration,
    config: &BuildConfig,
    mutated_source: &str,
    original_file: &Path,
) -> Result<(), anyhow::Error> {
    // Find the root for the package
    let root = SourcePackageLayout::try_find_root(&original_file.canonicalize()?)?;

    // Get the relative path to the original file
    let relative_path = original_file.canonicalize()?;
    let relative_path = relative_path.strip_prefix(&root)?;

    let tempdir = tempfile::tempdir()?;

    // Copy the whole package to the tempdir
    // We need to copy the whole package because the Move compiler needs to find the Move.toml file and all the dependencies
    // as we don't know which files are needed for the compilation
    copy_dir_all(&root, &tempdir.path())?;

    // Write the mutated source to the tempdir in place of the original file
    std::fs::write(tempdir.path().join(relative_path), mutated_source)?;

    let mut output: Box<dyn Write> = if mutator_config.project.verbose {
        Box::new(std::io::stdout())
    } else {
        Box::new(std::io::sink())
    };

    // Compile the package
    config
        .clone()
        .compile_package(&tempdir.path(), &mut output)?;

    Ok(())
}

/// Copies all files and directories from the source directory to the destination directory.
///
/// # Arguments
///
/// * `src` - the source directory.
/// * `dst` - the destination directory.
///
/// # Returns
///
/// * `io::Result<()>` - Ok if the copy is successful, or an error if any error occurs.
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    if !dst.as_ref().exists() {
        fs::create_dir_all(dst.as_ref())?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn copy_dir_all_copies_all_files_and_directories() {
        let temp_dir = tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dst_dir = temp_dir.path().join("dst");

        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("file.txt"), "Hello, world!").unwrap();

        let result = copy_dir_all(&src_dir, &dst_dir);
        assert!(result.is_ok());
        assert!(dst_dir.join("file.txt").exists());
    }

    #[test]
    fn copy_dir_all_errors_if_source_does_not_exist() {
        let temp_dir = tempdir().unwrap();
        let src_dir = temp_dir.path().join("non_existent_src");
        let dst_dir = temp_dir.path().join("dst");

        let result = copy_dir_all(&src_dir, &dst_dir);
        assert!(result.is_err());
    }
}