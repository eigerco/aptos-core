use clap::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_OUTPUT_DIR: &str = "mutants_output";

/// Command line options for mutator
#[derive(Parser, Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// The paths to the Move sources.
    #[clap(long, short, value_parser)]
    pub move_sources: Vec<PathBuf>,
    /// The paths to the Move sources to include.
    #[clap(long, short, value_parser)]
    pub include_only_files: Option<Vec<PathBuf>>,
    /// The paths to the Move sources to exclude.
    #[clap(long, short, value_parser)]
    pub exclude_files: Option<Vec<PathBuf>>,
    /// The path where to put the output files.
    #[clap(long, short, value_parser)]
    pub out_mutant_dir: Option<PathBuf>,
    /// Indicates if mutants should be verified and made sure mutants can compile.
    #[clap(long, default_value = "false")]
    pub verify_mutants: bool,
    /// Indicates if the output files should be overwritten.
    #[clap(long, short)]
    pub no_overwrite: Option<bool>,
    /// Name of the filter to use for down sampling.
    #[clap(long)]
    pub downsample_filter: Option<String>,
    /// Optional configuration file. If provided, it will override the default configuration.
    #[clap(long, short, value_parser)]
    pub configuration_file: Option<PathBuf>,
}

impl Default for Options {
    // We need to implement default just because we need to specify the default value for out_mutant_dir.
    // Otherwise, out_mutant_dir would be empty. This is special case, when user won't specify any Options
    // (so the default value would be used), but define package_path (which is passed using other mechanism).
    fn default() -> Self {
        Self {
            move_sources: vec![],
            include_only_files: None,
            exclude_files: None,
            out_mutant_dir: Some(PathBuf::from(DEFAULT_OUTPUT_DIR)),
            verify_mutants: true,
            no_overwrite: None,
            downsample_filter: None,
            configuration_file: None,
        }
    }
}
