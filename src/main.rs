mod commands;
mod config;
mod tokenizer;
mod utils;

use crate::commands::{CombineOptions, run_combine, run_file, run_init, run_structure, run_tokenize, run_get};
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

    /// Clone repository and combine its files (shortcut for get subcommand)
    #[arg(short = 'g', long = "get", num_args(1..=2), value_names = ["URL", "DIR"])]
    get: Option<Vec<String>>,

    /// Show project structure (shortcut for structure subcommand)
    #[arg(short = 's', long = "structure")]
    structure: bool,

    /// Show merged file contents (shortcut for file subcommand)
    #[arg(short = 'f', long = "file")]
    file: bool,

    /// Update an existing merge file (shortcut for update subcommand)
    #[arg(short = 'u', long = "update", value_name = "DIR")]
    update: Option<Option<PathBuf>>,

    /// Tokenize a file using all available tokenizers
    #[arg(short = 't', long = "tokenize", value_name = "FILE", num_args(0..=1))]
    tokenize: Option<Option<PathBuf>>,

    /// Split option: if token limit is exceeded, split into parts. Value for limit (e.g. 350K, 1.2M, default 500K)
    #[arg(long = "split", value_name = "LIMIT", default_missing_value = "500K", num_args = 0..=1, global = true)]
    split: Option<String>,

    /// Do not split option: ignore limit, write all to one file (takes precedence over split check)
    #[arg(long = "notsplit", global = true)]
    notsplit: bool,

    /// Ignore size check for individual files (> 100 KB)
    #[arg(short = 'i', long = "ignore", global = true)]
    ignore: bool,

    /// Prompt to choose processing pattern interactively
    #[arg(short = 'p', long = "pattern", global = true)]
    pattern: bool,

    /// Use Full processing mode (default)
    #[arg(long = "pattern-full", global = true)]
    pattern_full: bool,

    /// Use Minify processing mode (removes comments and extra spaces)
    #[arg(long = "pattern-min", global = true)]
    pattern_min: bool,

    /// Use Maximize processing mode (signatures/skeletons only). filters: d="dir" f="file"
    #[arg(long = "pattern-max", num_args(0..), value_name = "FILTERS", global = true)]
    pattern_max: Option<Vec<String>>,
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
    /// Clone repository and combine its files
    Get {
        /// Repository URL
        url: String,
        /// Target directory for output file
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

    let options = CombineOptions {
        is_update: false,
        split: cli.split.clone(),
        notsplit: cli.notsplit,
        ignore_size: cli.ignore,
        pattern: cli.pattern,
        pattern_full: cli.pattern_full,
        pattern_min: cli.pattern_min,
        pattern_max: cli.pattern_max.clone(),
        custom_project_name: None,
        custom_output_dir: None,
    };

    // Handle shortcuts
    if let Some(tokenize_opt) = cli.tokenize {
        let file_path = match tokenize_opt {
            Some(path) => Some(path),
            None => None,
        };
        return run_tokenize(file_path);
    }

    if let Some(get_args) = cli.get {
        let url = get_args[0].clone();
        let dir = if get_args.len() > 1 {
            Some(PathBuf::from(&get_args[1]))
        } else {
            None
        };
        return run_get(&url, dir, options);
    }

    if let Some(dir_opt) = cli.combine {
        let dir = match dir_opt {
            Some(d) => d,
            None => select_directory()?,
        };
        return run_combine(dir, options);
    }
    if let Some(dir_opt) = cli.update {
        let dir = match dir_opt {
            Some(d) => d,
            None => select_directory()?,
        };
        let mut opts = options.clone();
        opts.is_update = true;
        return run_combine(dir, opts);
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
            run_combine(dir, options)
        }
        Some(Commands::Get { url, dir }) => {
            run_get(&url, dir, options)
        }
        Some(Commands::Update { dir }) => {
            let dir = match dir {
                Some(d) => d,
                None => select_directory()?,
            };
            let mut opts = options.clone();
            opts.is_update = true;
            run_combine(dir, opts)
        }
        Some(Commands::Structure) => run_structure(),
        Some(Commands::File) => run_file(),
        None => {
            println!("Use 'mrg --help' for usage info.");
            Ok(())
        }
    }
}
