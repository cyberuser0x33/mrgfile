use ai_tokenizer_tools::AICounter;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Select, Confirm};
use ignore::gitignore::GitignoreBuilder;
use sha3::{Digest, Sha3_256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "mrg")]
#[command(about = "Project merger tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Combine project files (shortcut for combine subcommand)
    #[arg(short = 'c', long = "combine", value_name = "DIR")]
    combine: Option<PathBuf>,

    /// Show project structure (shortcut for structure subcommand)
    #[arg(short = 's', long = "structure")]
    structure: bool,

    /// Show merged file contents (shortcut for file subcommand)
    #[arg(short = 'f', long = "file")]
    file: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize .mrgignore file
    Init,
    /// Combine project files
    Combine {
        /// Target directory
        #[arg(value_name = "DIR")]
        dir: PathBuf,
    },
    /// Show project structure
    Structure,
    /// Show merged file contents
    File,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle shortcuts
    if let Some(dir) = cli.combine {
        return run_combine(dir);
    }
    if cli.structure {
        return run_structure();
    }
    if cli.file {
        return run_file();
    }

    match cli.command {
        Some(Commands::Init) => run_init(),
        Some(Commands::Combine { dir }) => run_combine(dir),
        Some(Commands::Structure) => run_structure(),
        Some(Commands::File) => run_file(),
        None => {
            println!("Use 'mrg --help' for usage info.");
            Ok(())
        }
    }
}

fn run_init() -> Result<()> {
    let path = Path::new(".mrgignore");
    if path.exists() {
        println!("[!] .mrgignore already exists.");
        return Ok(());
    }

    let default_ignore = r#"# --- SYSTEM AND HIDDEN FILES ---
.git
.gitignore
.mrgignore
.DS_Store
Thumbs.db
desktop.ini

# --- CONFIDENTIAL INFORMATION (Secrets) ---
.env
.env.local
.env.development.local
.env.test.local
.env.production.local
*.pem
*.key
*.pub
id_rsa
id_ed25519
secrets.yaml
auth.json
credentials.json
*.pfx
*.p12

# --- DEPENDENCY AND BUILD FOLDERS (Heavy/Build) ---
node_modules
bower_components
jspm_packages
venv
.venv
env
__pycache__
target
build
dist
out
release_files
Debug
Release
.gradle
ipch
.terraform

# --- IDE and Development Tools ---
.idea
.vscode
*.swp
*.swo
.eslintcache
.sass-cache
.cache

# --- BINARY FILES AND COMPILATION ---
*.pyc
*.pyo
*.pyd
*.o
*.obj
*.so
*.dll
*.dylib
*.class
*.jar
*.exe
*.bin
*.exp
*.lib
*.def
*.out
core

# --- ARCHIVES AND COMPRESSED DATA ---
*.zip
*.tar
*.gz
*.7z
*.rar
*.dmg
*.iso
*.apk
*.ipa

# --- DATABASES AND DATA (Non-text) ---
*.db
*.sqlite
*.sqlite3
*.pickle
*.pkl
*.h5
*.npy
*.parquet

# --- MEDIA AND DOCUMENTS (Binary) ---
*.png
*.jpg
*.jpeg
*.gif
*.ico
*.pdf
*.mp4
*.avi
*.mp3
*.ttf
*.otf
*.woff
*.woff2
*.eot

# --- LOGS AND INFORMATION NOISE ---
*.log
npm-debug.log*
yarn-debug.log*
yarn-error.log*
*.bak
*.tmp
*.temp
*.stackdump
mrg-*.txt

# --- CONFIGURATIONS THAT MAY BE UNNECESSARY ---
Cargo.lock
*.xml
package-lock.json
"#;

    fs::write(path, default_ignore)?;
    println!("[+] Created .mrgignore with detailed patterns.");
    Ok(())
}

fn run_combine(dir: PathBuf) -> Result<()> {
    println!("[*] Scanning directory: {:?}", dir);
    
    if !dir.exists() {
        anyhow::bail!("Directory {:?} does not exist.", dir);
    }

    // Check for .mrgignore and warn if missing
    let ignore_path = Path::new(".mrgignore");
    if !ignore_path.exists() {
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Warning: .mrgignore file not found in the project directory. All files will be merged. Continue?")
            .default(false)
            .interact()?;
        
        if !confirm {
            println!("[*] Operation cancelled by user.");
            return Ok(());
        }
    }

    // Load .mrgignore
    let mut builder = GitignoreBuilder::new(".");
    if ignore_path.exists() {
        builder.add(ignore_path);
    }
    let ignore_matcher = builder.build().context("Failed to build ignore matcher")?;

    let mut tree_lines = Vec::new();
    let mut files_to_merge = Vec::new();
    
    // Get project folder name for the root of the tree
    let root_name = dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".into());

    build_tree(&dir, &dir, "", &mut tree_lines, &mut files_to_merge, &ignore_matcher)?;

    let mut output = String::new();
    output.push_str("Merging Files v.0.1.3\n\n");
    output.push_str("Project Structure:\n");
    output.push_str(&format!("{}/\n", root_name));
    for line in &tree_lines {
        output.push_str(line);
        output.push('\n');
    }
    output.push_str("\n");
    output.push_str(&"=".repeat(30));
    output.push_str("\n\n");

    for (full_path, rel_path) in &files_to_merge {
        output.push_str(&format!("=== start {} ===\n", rel_path));
        match fs::read_to_string(full_path) {
            Ok(content) => {
                output.push_str(&content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            }
            Err(_) => {
                output.push_str("// [BINARY OR NON-UTF8 FILE SKIPPED]\n");
            }
        }
        output.push_str(&format!("=== end {} ===\n\n", rel_path));
    }

    // Hash the content
    let mut hasher = Sha3_256::new();
    hasher.update(output.as_bytes());
    let hash_result = hasher.finalize();
    let hash_hex = hex::encode(hash_result);
    let hash_short = &hash_hex[..8];

    let output_filename = format!("mrg-{}.txt", hash_short);
    fs::write(&output_filename, &output)?;

    println!("[+] Created {}", output_filename);
    println!("[*] Files merged: {}", files_to_merge.len());

    // AI Token counting
    let counter = AICounter::new("AItokenizers").map_err(|e| anyhow::anyhow!("AI Counter init error: {}", e))?;
    let res = counter.count_string(&output);
    
    println!("Words: {}, Characters: {}", res.words, res.chars);
    println!("SHA3-256-data: {}", hash_hex);
    println!("\nToken Statistics (there may be some margin of error): ");
    println!("~ {}", res.gpt_string);
    println!("~ {}", res.gemini_string);
    println!("~ {}", res.claude_string);

    Ok(())
}

fn build_tree(
    root: &Path,
    current_dir: &Path,
    prefix: &str,
    tree_lines: &mut Vec<String>,
    files_to_merge: &mut Vec<(PathBuf, String)>,
    ignore_matcher: &ignore::gitignore::Gitignore,
) -> Result<()> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        let rel_path = path.strip_prefix(root)?;
        let is_dir = path.is_dir();
        
        // Check ignore
        if ignore_matcher.matched(&rel_path, is_dir).is_ignore() {
            continue;
        }
        
        entries.push(entry);
    }

    // Sort: dirs first, then alphabetically
    entries.sort_by(|a, b| {
        let a_path = a.path();
        let b_path = b.path();
        let a_is_dir = a_path.is_dir();
        let b_is_dir = b_path.is_dir();
        
        if a_is_dir != b_is_dir {
            b_is_dir.cmp(&a_is_dir)
        } else {
            a.file_name().cmp(&b.file_name())
        }
    });

    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        
        tree_lines.push(format!("{}{}{}", prefix, connector, name_str));

        let rel_path = path.strip_prefix(root)?;
        let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");

        if path.is_dir() {
            let next_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            build_tree(root, &path, &next_prefix, tree_lines, files_to_merge, ignore_matcher)?;
        } else {
            files_to_merge.push((path, rel_path_str));
        }
    }

    Ok(())
}

fn run_structure() -> Result<()> {
    let file_path = select_mrg_file()?;
    let content = fs::read_to_string(file_path)?;
    
    if let Some(pos) = content.find("==========") {
        println!("{}", &content[..pos]);
    } else {
        println!("{}", content);
    }
    Ok(())
}

fn run_file() -> Result<()> {
    let file_path = select_mrg_file()?;
    let content = fs::read_to_string(file_path)?;
    
    if let Some(pos) = content.find("==========") {
        println!("{}", &content[pos + 30..].trim_start());
    } else {
        println!("{}", content);
    }
    Ok(())
}

fn select_mrg_file() -> Result<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(".")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("mrg-") && name.ends_with(".txt") {
            files.push(entry.path());
        }
    }

    if files.is_empty() {
        anyhow::bail!("No mrg-*.txt files found in the current directory.");
    }

    files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::now()));
    files.reverse(); // Newest first

    if files.len() == 1 {
        return Ok(files[0].clone());
    }

    let items: Vec<String> = files.iter()
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Multiple mrg-*.txt files found. Select one:")
        .items(&items)
        .default(0)
        .interact()?;

    Ok(files[selection].clone())
}
