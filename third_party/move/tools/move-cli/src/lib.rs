// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use base::{
    build::Build, coverage::Coverage, disassemble::Disassemble, docgen::Docgen, errmap::Errmap,
    movey_login::MoveyLogin, movey_upload::MoveyUpload, mutate::Mutate, new::New, prove::Prove,
    spec_test::SpecTest, test::Test,
};
use move_package::BuildConfig;

pub mod base;
pub mod sandbox;
pub mod utils;

/// Default directory where saved Move resources live
pub const DEFAULT_STORAGE_DIR: &str = "storage";

/// Default directory for build output
pub const DEFAULT_BUILD_DIR: &str = ".";

/// Extension for resource and event files, which are in BCS format
const BCS_EXTENSION: &str = "bcs";

use anyhow::Result;
use clap::Parser;
use move_core_types::{
    account_address::AccountAddress, effects::ChangeSet, errmap::ErrorMapping,
    identifier::Identifier,
};
use move_vm_runtime::native_functions::NativeFunction;
use move_vm_test_utils::gas_schedule::CostTable;
use std::path::PathBuf;

type NativeFunctionRecord = (AccountAddress, Identifier, Identifier, NativeFunction);

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Move {
    /// Path to a package which the command should be run with respect to.
    #[clap(long = "path", short = 'p', global = true, value_parser)]
    pub package_path: Option<PathBuf>,

    /// Print additional diagnostics if available.
    #[clap(short = 'v', global = true)]
    pub verbose: bool,

    /// Package build options
    #[clap(flatten)]
    pub build_config: BuildConfig,
}

/// MoveCLI is the CLI that will be executed by the `move-cli` command
/// The `cmd` argument is added here rather than in `Move` to make it
/// easier for other crates to extend `move-cli`
#[derive(Parser)]
pub struct MoveCLI {
    #[clap(flatten)]
    pub move_args: Move,

    #[clap(subcommand)]
    pub cmd: Command,
}

#[derive(Parser)]
pub enum Command {
    Build(Build),
    Coverage(Coverage),
    Disassemble(Disassemble),
    Docgen(Docgen),
    Errmap(Errmap),
    MoveyUpload(MoveyUpload),
    Mutate(Mutate),
    New(New),
    Prove(Prove),
    SpecTest(SpecTest),
    Test(Test),
    /// Execute a sandbox command.
    #[clap(name = "sandbox")]
    Sandbox {
        /// Directory storing Move resources, events, and module bytecodes produced by module publishing
        /// and script execution.
        #[clap(long, default_value = DEFAULT_STORAGE_DIR, value_parser)]
        storage_dir: PathBuf,
        #[clap(subcommand)]
        cmd: sandbox::cli::SandboxCommand,
    },
    #[clap(name = "movey-login")]
    MoveyLogin(MoveyLogin),
}

pub fn run_cli(
    natives: Vec<NativeFunctionRecord>,
    genesis: ChangeSet,
    cost_table: &CostTable,
    error_descriptions: &ErrorMapping,
    move_args: Move,
    cmd: Command,
) -> Result<()> {
    // TODO: right now, the gas metering story for move-cli (as a library) is a bit of a mess.
    //         1. It's still using the old CostTable.
    //         2. The CostTable only affects sandbox runs, but not unit tests, which use a unit cost table.
    match cmd {
        Command::Build(c) => c.execute(move_args.package_path, move_args.build_config),
        Command::Coverage(c) => c.execute(move_args.package_path, move_args.build_config),
        Command::Disassemble(c) => c.execute(move_args.package_path, move_args.build_config),
        Command::Docgen(c) => c.execute(move_args.package_path, move_args.build_config),
        Command::Errmap(c) => c.execute(move_args.package_path, move_args.build_config),
        Command::MoveyUpload(c) => c.execute(move_args.package_path),
        Command::Mutate(c) => c.execute(move_args.package_path, move_args.build_config),
        Command::New(c) => c.execute_with_defaults(move_args.package_path),
        Command::Prove(c) => c.execute(move_args.package_path, move_args.build_config),
        Command::SpecTest(c) => c.execute(move_args.package_path, move_args.build_config),
        Command::Test(c) => c.execute(
            move_args.package_path,
            move_args.build_config,
            natives,
            genesis,
            Some(cost_table.clone()),
        ),
        Command::Sandbox { storage_dir, cmd } => cmd.handle_command(
            natives,
            cost_table,
            error_descriptions,
            &move_args,
            &storage_dir,
        ),
        Command::MoveyLogin(c) => c.execute(),
    }
}

pub fn move_cli(
    natives: Vec<NativeFunctionRecord>,
    genesis: ChangeSet,
    cost_table: &CostTable,
    error_descriptions: &ErrorMapping,
) -> Result<()> {
    let args = MoveCLI::parse();
    run_cli(
        natives,
        genesis,
        cost_table,
        error_descriptions,
        args.move_args,
        args.cmd,
    )
}

#[test]
fn verify_tool() {
    use clap::CommandFactory;
    MoveCLI::command().debug_assert()
}
