//! Main entry point for the Ozymandias CLI application.
//!
//! This file defines the command-line interface, argument parsing, and top-level command dispatch for running and validating service scenario files.
//!
//! # Usage
//! Run the CLI with `run` or `validate` subcommands, providing a scenario file or directory.
//!
//! ```sh
//! ozymandias run ./scenarios
//! ozymandias validate ./scenarios/1.toml
//! ```

use clap::{Parser, Subcommand};
use logging::setup_logging;
use tracing::info;

use crate::collect_toml::collect_toml_paths;

mod collect_toml;
mod errors;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ozymandias - a container-based integration test runner for service scenarios
#[derive(Parser, Debug)]
#[command(author = "Anit Nilay", version = VERSION, about, long_about = None)]
pub struct Cli {
    /// Enable verbose logging (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Path to output report file (e.g., HTML or JUnit XML)
    #[arg(short, long, value_name = "FILE")]
    pub report: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a scenario TOML file or directory of scenarios
    Run {
        /// Path to a scenario TOML file or directory containing multiple TOML files
        #[arg(value_name = "SCENARIO_PATH")]
        scenario_path: String,
    },

    /// Validate a scenario file without running containers or tests
    Validate {
        /// Path to the scenario TOML file
        #[arg(value_name = "SCENARIO_PATH")]
        scenario_path: String,
    },
}

/// Main async entry point for the CLI application.
///
/// Parses command-line arguments, sets up logging, and dispatches to the appropriate subcommand handler.
///
/// # Errors
/// Returns an error if argument parsing, logging setup, or scenario collection fails.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    setup_logging(cli.verbose)?;

    match &cli.command {
        Commands::Run { scenario_path } => {
            info!("Running scenario(s): {scenario_path}");
            let paths = collect_toml_paths(scenario_path)?;
            for path in paths {
                info!("Executing scenario: {}", path.display());
                // TODO: Run scenario
            }
        }
        Commands::Validate { scenario_path } => {
            info!("Validating scenario(s): {scenario_path}");
            let paths = collect_toml_paths(scenario_path)?;
            for path in paths {
                info!("Validating: {}", path.display());
                // TODO: Validate scenario
            }
        }
    };

    Ok(())
}
