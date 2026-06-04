use anyhow::Result;
use chrono::Utc;
use dialoguer::{theme::ColorfulTheme, Select};
use sha3::{Digest, Sha3_256};
use std::fs;
use std::path::{Path, PathBuf};

pub fn build_tree(
    root: &Path,
    current_dir: &Path,
    prefix: &str,
    tree_lines: &mut Vec<String>,
    files_to_merge: &mut Vec<(PathBuf, String)>,
    ignore_matcher: &ignore::gitignore::Gitignore,
    ignored_count: &mut usize,
) -> Result<()> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        let rel_path = path.strip_prefix(root)?;
        let is_dir = path.is_dir();
        
        if ignore_matcher.matched(&rel_path, is_dir).is_ignore() {
            *ignored_count += 1;
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
            build_tree(root, &path, &next_prefix, tree_lines, files_to_merge, ignore_matcher, ignored_count)?;
        } else {
            files_to_merge.push((path, rel_path_str));
        }
    }

    Ok(())
}

pub fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}

pub fn get_current_timestamp() -> String {
    // DD-Mon-YYYY/HH:MM:SS
    Utc::now().format("%d-%b-%Y/%H:%M:%S").to_string()
}

pub fn calculate_sha3_256(data: &str) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn select_directory() -> Result<PathBuf> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(".")? {
        let entry = entry?;
        if entry.path().is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') && name != "target" {
                dirs.push(entry.path());
            }
        }
    }

    if dirs.is_empty() {
        anyhow::bail!("No project directories found in the current directory.");
    }

    let items: Vec<String> = dirs.iter()
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a project directory:")
        .items(&items)
        .default(0)
        .interact()?;

    Ok(dirs[selection].clone())
}

pub fn select_mrg_file() -> Result<PathBuf> {
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
    files.reverse();

    if files.len() == 1 {
        return Ok(files[0].clone());
    }

    let items: Vec<String> = files.iter()
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select an mrg file:")
        .items(&items)
        .default(0)
        .interact()?;

    Ok(files[selection].clone())
}
