use crate::config::DEFAULT_IGNORE_CONTENT;
use crate::tokenizer::AICounter;
use crate::utils::{
    build_tree, calculate_sha3_256, format_file_size, get_current_timestamp, select_mrg_file,
};
use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm};
use ignore::gitignore::GitignoreBuilder;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run_init(project_name: Option<String>) -> Result<()> {
    let path = Path::new(".mrgignore");
    if path.exists() {
        println!("[!] .mrgignore already exists.");
        return Ok(());
    }

    fs::write(path, DEFAULT_IGNORE_CONTENT)?;
    let msg = match project_name {
        Some(name) => format!("[+] Created .mrgignore for project '{}'.", name),
        None => "[+] Created .mrgignore with default patterns.".to_string(),
    };
    println!("{}", msg);
    Ok(())
}

pub fn run_combine(dir: PathBuf, is_update: bool) -> Result<()> {
    println!("[*] Scanning directory: {:?}", dir);

    if !dir.exists() {
        anyhow::bail!("Directory {:?} does not exist.", dir);
    }

    let ignore_path = Path::new(".mrgignore");
    if !ignore_path.exists() {
        println!(
            "WARNING custom file .mrgignore not found. Use command 'mrg init projectName' for initialization."
        );
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("The default configuration file for the current version of the program will now be used. Do you want to continue?")
            .default(false)
            .interact()?;

        if !confirm {
            println!("[*] Operation cancelled by user.");
            return Ok(());
        }
        // Create standard .mrgignore immediately and use it
        run_init(None)?;
    }

    let mut builder = GitignoreBuilder::new(".");
    builder.add(ignore_path);
    let ignore_matcher = builder.build().context("Failed to build ignore matcher")?;

    let mut tree_lines = Vec::new();
    let mut files_to_merge = Vec::new();
    let mut ignored_count = 0;

    let root_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".into());

    build_tree(
        &dir,
        &dir,
        "",
        &mut tree_lines,
        &mut files_to_merge,
        &ignore_matcher,
        &mut ignored_count,
    )?;

    let mut content_to_hash = String::new();
    content_to_hash.push_str("Project Structure:\n");
    content_to_hash.push_str(&format!("{}/\n", root_name));
    for line in &tree_lines {
        content_to_hash.push_str(line);
        content_to_hash.push('\n');
    }
    content_to_hash.push('\n');
    content_to_hash.push_str(&"=".repeat(30));
    content_to_hash.push_str("\n\n");

    for (full_path, rel_path) in &files_to_merge {
        content_to_hash.push_str(&format!("=== start {} ===\n", rel_path));
        match fs::read_to_string(full_path) {
            Ok(content) => {
                content_to_hash.push_str(&content);
                if !content.ends_with('\n') {
                    content_to_hash.push('\n');
                }
            }
            Err(_) => {
                content_to_hash.push_str("non supported data, skipped\n");
            }
        }
        content_to_hash.push_str(&format!("=== end {} ===\n\n", rel_path));
    }

    let hash_hex = calculate_sha3_256(&content_to_hash);
    let version = env!("CARGO_PKG_VERSION");
    let timestamp = get_current_timestamp();

    let mut final_output = String::new();
    final_output.push_str(&format!("Project merger tool v{}\n", version));
    final_output.push_str(&format!("{} ({})\n", root_name, timestamp));
    final_output.push_str(&format!("hash(sha3-256):{}\n", hash_hex));
    final_output.push_str("**********\n");
    final_output.push_str(&content_to_hash);

    let output_filename = format!("mrg-{}.txt", root_name);
    
    if Path::new(&output_filename).exists() && !is_update {
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("File {} already exists. Overwrite?", output_filename))
            .default(false)
            .interact()?;
        
        if !confirm {
            println!("[*] Operation cancelled. Output file not saved.");
            return Ok(());
        }
    }

    fs::write(&output_filename, &final_output)?;
    let metadata = fs::metadata(&output_filename)?;
    let size_str = format_file_size(metadata.len());

    println!("[+] Created {} ({})", output_filename, size_str);
    println!("[*] Files merged: {}", files_to_merge.len());
    println!("[*] Files ignored: {}", ignored_count);

    // AI Token counting
    match AICounter::new("AItokenizers") {
        Ok(counter) => {
            let res = counter.count_string(&final_output);
            println!("Words: {}, Characters: {}", res.words, res.chars);
            println!("SHA3-256-data: {}", hash_hex);
            println!("\nToken Statistics (there may be some margin of error): ");
            println!("~ {}", res.gpt_string);
            println!("~ {}", res.gemini_string);
            println!("~ {}", res.claude_string);
        }
        Err(e) => {
            println!("[!] AI Counter error: {}", e);
        }
    }

    Ok(())
}

pub fn run_structure() -> Result<()> {
    let file_path = select_mrg_file()?;
    let content = fs::read_to_string(file_path)?;

    if let Some(pos) = content.find("**********") {
        let after_sep = &content[pos + 10..].trim_start();
        if let Some(end_pos) = after_sep.find("==========") {
            println!("{}", &after_sep[..end_pos]);
        } else {
            println!("{}", after_sep);
        }
    } else {
        println!("{}", content);
    }
    Ok(())
}

pub fn run_file() -> Result<()> {
    let file_path = select_mrg_file()?;
    let content = fs::read_to_string(file_path)?;

    if let Some(pos) = content.find("**********") {
        let after_sep = &content[pos + 10..].trim_start();
        if let Some(end_pos) = after_sep.find("==========") {
            println!("{}", &after_sep[end_pos + 30..].trim_start());
        } else {
            println!("{}", after_sep);
        }
    } else {
        println!("{}", content);
    }
    Ok(())
}
