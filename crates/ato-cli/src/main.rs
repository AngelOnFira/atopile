//! Atopile compiler CLI.
//!
//! The main `ato` binary that orchestrates compilation of .ato files.

mod commands;
mod error;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

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
        /// Path to .ato file or build target name from ato.yaml.
        /// If omitted, uses the "default" target from ato.yaml.
        #[arg(value_name = "TARGET")]
        target: Option<String>,

        /// Output directory.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Show verbose output.
        #[arg(short, long)]
        verbose: bool,

        /// Skip automatic package installation.
        #[arg(long)]
        no_install: bool,
    },

    /// Search and query the parts database.
    Parts {
        #[command(subcommand)]
        action: PartsAction,
    },

    /// Add a package dependency.
    Add {
        /// Package to add (e.g., "atopile/generics", "atopile/generics@1.0.0").
        #[arg(value_name = "PACKAGE")]
        package: String,

        /// Path to the project directory.
        #[arg(short = 'C', long)]
        project_path: Option<PathBuf>,
    },

    /// Install all dependencies from ato.yaml.
    Install {
        /// Path to the project directory.
        #[arg(short = 'C', long)]
        project_path: Option<PathBuf>,
    },

    /// Update dependencies to latest versions.
    Update {
        /// Path to the project directory.
        #[arg(short = 'C', long)]
        project_path: Option<PathBuf>,
    },

    /// Remove a package dependency.
    Remove {
        /// Package to remove.
        #[arg(value_name = "PACKAGE")]
        package: String,

        /// Path to the project directory.
        #[arg(short = 'C', long)]
        project_path: Option<PathBuf>,
    },

    /// List installed dependencies.
    List {
        /// Path to the project directory.
        #[arg(short = 'C', long)]
        project_path: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PartsAction {
    /// Search for parts in the LCSC database.
    Search {
        /// Search query (e.g., "10k 0402 resistor", "100nF capacitor").
        #[arg(value_name = "QUERY")]
        query: String,

        /// Component type filter (resistor, capacitor, inductor, etc.).
        #[arg(short = 't', long)]
        component_type: Option<String>,

        /// Package filter (0402, 0603, 0805, etc.).
        #[arg(short, long)]
        package: Option<String>,

        /// Maximum number of results.
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Fetch a specific part by LCSC ID.
    Fetch {
        /// LCSC part ID (e.g., "C25871" or just "25871").
        #[arg(value_name = "LCSC_ID")]
        lcsc_id: String,
    },

    /// Show cache information.
    CacheInfo,

    /// Clear the parts cache.
    CacheClear,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Parse { file, format } => commands::parse::run(&file, &format),
        Commands::Check { file, verbose } => commands::check::run(&file, verbose),
        Commands::Build { target, output, verbose, no_install } => {
            commands::build::run_with_options(target.as_deref(), output.as_deref(), verbose, no_install)
        }
        Commands::Parts { action } => match action {
            PartsAction::Search {
                query,
                component_type,
                package,
                limit,
            } => commands::parts::search(&query, component_type.as_deref(), package.as_deref(), limit),
            PartsAction::Fetch { lcsc_id } => commands::parts::fetch(&lcsc_id),
            PartsAction::CacheInfo => commands::parts::cache_info(),
            PartsAction::CacheClear => commands::parts::cache_clear(),
        },
        Commands::Add { package, project_path } => {
            commands::package::add(&package, project_path.as_deref())
        }
        Commands::Install { project_path } => {
            commands::package::install(project_path.as_deref())
        }
        Commands::Update { project_path } => {
            commands::package::update(project_path.as_deref())
        }
        Commands::Remove { package, project_path } => {
            commands::package::remove(&package, project_path.as_deref())
        }
        Commands::List { project_path } => {
            commands::package::list(project_path.as_deref())
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            e.report();
            ExitCode::FAILURE
        }
    }
}
