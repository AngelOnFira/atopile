//! Atopile compiler CLI.
//!
//! The main `ato` binary that orchestrates compilation of .ato files.

mod commands;
mod error;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use error::CliResult;

/// Atopile - A declarative language for designing electronics.
#[derive(Parser)]
#[command(name = "ato")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse and dump AST (for debugging).
    Parse {
        /// Path to the .ato file to parse.
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output format (json or pretty).
        #[arg(short, long, default_value = "pretty")]
        format: String,
    },

    /// Check a file without full build (semantic analysis only).
    Check {
        /// Path to the .ato file to check.
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Show verbose output.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Build a project (full compilation).
    Build {
        /// Path to the .ato file to build.
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output directory.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Show verbose output.
        #[arg(short, long)]
        verbose: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Parse { file, format } => commands::parse::run(&file, &format),
        Commands::Check { file, verbose } => commands::check::run(&file, verbose),
        Commands::Build { file, output, verbose } => commands::build::run(&file, output.as_deref(), verbose),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            e.report();
            ExitCode::FAILURE
        }
    }
}
