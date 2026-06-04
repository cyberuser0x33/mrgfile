mod commands;
mod config;
mod tokenizer;
mod utils;

use crate::commands::{run_combine, run_file, run_init, run_structure};
use crate::utils::select_directory;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mrg")]
#[command(about = "Project merger tool", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Combine project files (shortcut for combine subcommand)
    #[arg(short = 'c', long = "combine", value_name = "DIR")]
    combine: Option<Option<PathBuf>>,

    /// Show project structure (shortcut for structure subcommand)
    #[arg(short = 's', long = "structure")]
    structure: bool,

    /// Show merged file contents (shortcut for file subcommand)
    #[arg(short = 'f', long = "file")]
    file: bool,

    /// Update an existing merge file (shortcut for update subcommand)
    #[arg(short = 'u', long = "update", value_name = "DIR")]
    update: Option<Option<PathBuf>>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize .mrgignore file
    Init {
        /// Project name for the ignore file
        #[arg(value_name = "NAME")]
        name: Option<String>,
    },
    /// Combine project files
    Combine {
        /// Target directory
        #[arg(value_name = "DIR")]
        dir: Option<PathBuf>,
    },
    /// Show project structure
    Structure,
    /// Show merged file contents
    File,
    /// Update an existing merge file
    Update {
        /// Target directory
        #[arg(value_name = "DIR")]
        dir: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle shortcuts
    if let Some(dir_opt) = cli.combine {
        let dir = match dir_opt {
            Some(d) => d,
            None => select_directory()?,
        };
        return run_combine(dir, false);
    }
    if let Some(dir_opt) = cli.update {
        let dir = match dir_opt {
            Some(d) => d,
            None => select_directory()?,
        };
        return run_combine(dir, true);
    }
    if cli.structure {
        return run_structure();
    }
    if cli.file {
        return run_file();
    }

    match cli.command {
        Some(Commands::Init { name }) => run_init(name),
        Some(Commands::Combine { dir }) => {
            let dir = match dir {
                Some(d) => d,
                None => select_directory()?,
            };
            run_combine(dir, false)
        }
        Some(Commands::Update { dir }) => {
            let dir = match dir {
                Some(d) => d,
                None => select_directory()?,
            };
            run_combine(dir, true)
        }
        Some(Commands::Structure) => run_structure(),
        Some(Commands::File) => run_file(),
        None => {
            println!("Use 'mrg --help' for usage info.");
            Ok(())
        }
    }
}
