use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;
use tracing::info;

mod logging;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    logging::setup_logging(cli.verbose)?;

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

/// Collect all `.toml` files if path is a directory, or just return the file path
fn collect_toml_paths<P: AsRef<Path>>(path: P) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    let path = path.as_ref();
    if path.is_dir() {
        let mut toml_files = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                toml_files.push(path);
            }
        }
        Ok(toml_files)
    } else {
        Ok(vec![path.to_path_buf()])
    }
}
