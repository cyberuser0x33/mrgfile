use crate::config::DEFAULT_IGNORE_CONTENT;
use crate::tokenizer::AICounter;
use crate::utils::{
    MaximizeFilters, ProcessingMode, TreeNode, calculate_sha3_256, format_file_size,
    get_current_timestamp, maximize_content, minify_content, select_mrg_file,
    is_binary_file,
};
use anyhow::Result;
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

#[derive(Clone, Debug)]
pub struct CombineOptions {
    pub is_update: bool,
    pub split: Option<Option<String>>,
    pub notsplit: bool,
    pub ignore_size: bool,
    pub pattern: bool,
    pub pattern_full: bool,
    pub pattern_min: bool,
    pub pattern_max: Option<Vec<String>>,
}

struct FileResult {
    rel_path: String,
    content: String,
    gpt_tokens: usize,
    gemini_tokens: usize,
    claude_tokens: usize,
    words: usize,
    chars: usize,
}

// ignore visitor implementation
struct ProjectVisitor {
    tx: mpsc::Sender<Result<ignore::DirEntry, ignore::Error>>,
}

impl ignore::ParallelVisitor for ProjectVisitor {
    fn visit(&mut self, entry: Result<ignore::DirEntry, ignore::Error>) -> ignore::WalkState {
        let _ = self.tx.send(entry);
        ignore::WalkState::Continue
    }
}

struct ProjectVisitorBuilder {
    tx: mpsc::Sender<Result<ignore::DirEntry, ignore::Error>>,
}

impl<'s> ignore::ParallelVisitorBuilder<'s> for ProjectVisitorBuilder {
    fn build(&mut self) -> Box<dyn ignore::ParallelVisitor> {
        Box::new(ProjectVisitor {
            tx: self.tx.clone(),
        })
    }
}

fn parse_limit(s: &str) -> Result<usize> {
    let s = s.trim().to_uppercase();
    if s.ends_with('K') {
        let val: f64 = s[..s.len() - 1].parse()?;
        Ok((val * 1000.0) as usize)
    } else if s.ends_with('M') {
        let val: f64 = s[..s.len() - 1].parse()?;
        Ok((val * 1_000_000.0) as usize)
    } else {
        let val: usize = s.parse()?;
        Ok(val)
    }
}

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

fn update_gitignore(_root_name: &str) -> Result<()> {
    let gitignore_path = Path::new(".gitignore");
    if gitignore_path.exists() {
        let content = fs::read_to_string(gitignore_path)?;
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

        let mrgignore_pattern = ".mrgignore".to_string();
        let txt_pattern = "mrg-*.txt".to_string();
        let dir_pattern = "mrg-*/".to_string();

        let has_mrgignore = lines.iter().any(|l| l.trim() == mrgignore_pattern);
        let has_txt = lines.iter().any(|l| l.trim() == txt_pattern);
        let has_dir = lines.iter().any(|l| l.trim() == dir_pattern);

        let mut modified = false;
        if !has_mrgignore {
            lines.push(mrgignore_pattern);
            modified = true;
        }
        if !has_txt {
            lines.push(txt_pattern);
            modified = true;
        }
        if !has_dir {
            lines.push(dir_pattern);
            modified = true;
        }

        if modified {
            let mut new_content = lines.join("\n");
            if !new_content.ends_with('\n') {
                new_content.push('\n');
            }
            fs::write(gitignore_path, new_content)?;
            println!("[+] Updated .gitignore with ignore patterns.");
        }
    }
    Ok(())
}

fn format_count(val: usize) -> String {
    let s = val.to_string();
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut result = String::new();
    for (i, &b) in bytes.iter().enumerate() {
        result.push(b as char);
        let rem = len - 1 - i;
        if rem > 0 && rem % 3 == 0 {
            result.push(',');
        }
    }
    result
}

pub fn run_combine(dir: PathBuf, options: CombineOptions) -> Result<()> {
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
            .interact()
            .map_err(anyhow::Error::from)
            .or_else(crate::utils::handle_interact_error)?;

        if !confirm {
            println!("[*] Operation cancelled by user.");
            return Ok(());
        }
        run_init(None)?;
    }

    // Determine processing mode
    let global_mode = if options.pattern {
        let patterns = vec![
            "Full (Keep entire code contents)",
            "Minify (Remove comments and extra whitespaces)",
            "Maximize (Extract only function/class/struct signatures/skeletons)",
        ];
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Choose processing pattern:")
            .items(&patterns)
            .default(0)
            .interact()
            .map_err(anyhow::Error::from)
            .or_else(crate::utils::handle_interact_error)?;
        match selection {
            0 => ProcessingMode::Full,
            1 => ProcessingMode::Minify,
            2 => ProcessingMode::Maximize,
            _ => ProcessingMode::Full,
        }
    } else if options.pattern_full {
        ProcessingMode::Full
    } else if options.pattern_min {
        ProcessingMode::Minify
    } else if options.pattern_max.is_some() {
        ProcessingMode::Maximize
    } else {
        ProcessingMode::Full
    };

    let maximize_filters = if global_mode == ProcessingMode::Maximize {
        if let Some(ref filters) = options.pattern_max {
            MaximizeFilters::parse(filters)
        } else {
            MaximizeFilters::default()
        }
    } else {
        MaximizeFilters::default()
    };

    let root_name = dir
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".into())
        });

    if Path::new(".gitignore").exists() {
        if let Err(e) = update_gitignore(&root_name) {
            eprintln!("[!] Failed to update .gitignore: {}", e);
        }
    }

    // 1. Parallel Scanning using WalkParallel
    let (tx, rx) = mpsc::channel();
    let mut builder = WalkBuilder::new(&dir);
    builder.standard_filters(true);
    builder.add_custom_ignore_filename(".mrgignore");

    if Path::new(".mrgignore").exists() {
        if let Some(e) = builder.add_ignore(".mrgignore") {
            eprintln!("[!] Error loading .mrgignore: {}", e);
        }
    }
    let walker = builder.build_parallel();

    let mut visitor_builder = ProjectVisitorBuilder { tx };
    walker.visit(&mut visitor_builder);
    drop(visitor_builder); // Close channel on main side

    let mut files = Vec::new();
    let mut dirs = Vec::new();

    for entry_res in rx {
        if let Ok(entry) = entry_res {
            let path = entry.path().to_path_buf();
            if path == dir {
                continue;
            }
            let rel_path = match path.strip_prefix(&dir) {
                Ok(p) => p.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                dirs.push((path, rel_path));
            } else {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((path, rel_path, size));
            }
        }
    }

    // 2. Filter out large files
    let mut final_files = Vec::new();
    let mut user_ignored_count = 0;

    for (path, rel_path, size) in files {
        if size > 100 * 1024 && !options.ignore_size {
            let size_str = format_file_size(size);
            let confirm = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "File {} is larger than 100 KB ({}). Include it?",
                    rel_path, size_str
                ))
                .default(false)
                .interact()
                .map_err(anyhow::Error::from)
                .or_else(crate::utils::handle_interact_error)?;
            if !confirm {
                user_ignored_count += 1;
                continue;
            }
        }
        final_files.push((path, rel_path));
    }

    let total_ignored_count = user_ignored_count;

    // 3. Reconstruct tree structure
    let mut root_tree = TreeNode::new(root_name.clone(), true);
    for (_, rel_path) in &dirs {
        root_tree.insert(Path::new(rel_path), true);
    }
    for (_, rel_path) in &final_files {
        root_tree.insert(Path::new(rel_path), false);
    }
    root_tree.sort();
    let mut tree_lines = Vec::new();
    root_tree.build_lines("", &mut tree_lines);

    // Initialize Tokenizer (load_all = false)
    let ai_counter =
        std::sync::Arc::new(AICounter::new("AItokenizers", false).map_err(|e| anyhow::anyhow!("{}", e))?);

    // 4. Parallel file processing (Rayon)
    println!("[*] Processing and tokenizing files...");
    let pb = ProgressBar::new(final_files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "[{elapsed_precise}] [{bar:40.green/white}] Merged {pos}/{len} files... {msg}",
            )
            .unwrap()
            .progress_chars("▰▰▱"),
    );

    let processed_files: Vec<FileResult> = final_files
        .par_iter()
        .map(|(path, rel_path)| {
            let mut file_content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => "non supported data, skipped\n".to_string(),
            };

            let mode = if global_mode == ProcessingMode::Maximize {
                if maximize_filters.matches(rel_path) {
                    ProcessingMode::Maximize
                } else {
                    ProcessingMode::Full
                }
            } else {
                global_mode
            };

            let extension = Path::new(rel_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            if mode == ProcessingMode::Minify {
                file_content = minify_content(&file_content, extension);
            } else if mode == ProcessingMode::Maximize {
                file_content = maximize_content(&file_content, extension);
            }

            let mut file_block = format!("=== start {} ===\n", rel_path);
            file_block.push_str(&file_content);
            if !file_block.ends_with('\n') {
                file_block.push('\n');
            }
            file_block.push_str(&format!("=== end {} ===\n\n", rel_path));

            let (gpt, gemini, claude) = ai_counter.count_tokens_raw(&file_block);
            let words = file_block.split_whitespace().count();
            let chars = file_block.chars().count();

            pb.inc(1);

            FileResult {
                rel_path: rel_path.clone(),
                content: file_content,
                gpt_tokens: gpt,
                gemini_tokens: gemini,
                claude_tokens: claude,
                words,
                chars,
            }
        })
        .collect();

    pb.finish_with_message("Done!");

    // Sort to ensure determinism
    let mut processed_files = processed_files;
    processed_files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let version = env!("CARGO_PKG_VERSION");
    let timestamp = get_current_timestamp();
    let output_filename = format!("mrg-{}.txt", root_name);

    if Path::new(&output_filename).exists() && !options.is_update {
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "File {} already exists. Overwrite?",
                output_filename
            ))
            .default(false)
            .interact()
            .map_err(anyhow::Error::from)
            .or_else(crate::utils::handle_interact_error)?;

        if !confirm {
            println!("[*] Operation cancelled. Output file not saved.");
            return Ok(());
        }
    }

    // 5. Memory-buffered Write and Incremental Hashing (No Seek)
    let mut body = String::new();
    body.push_str("Project Structure:\n");
    body.push_str(&format!("{}/\n", root_name));
    for line in &tree_lines {
        body.push_str(line);
        body.push('\n');
    }
    body.push('\n');

    for file_res in &processed_files {
        body.push_str(&format!("=== start {} ===\n", file_res.rel_path));
        body.push_str(&file_res.content);
        if !file_res.content.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&format!("=== end {} ===\n\n", file_res.rel_path));
    }

    let hash_hex = calculate_sha3_256(&body);
    let real_header = format!(
        "Project merger tool v{}\n{} ({})\nhash(sha3-256):{}\n**********\n",
        version, root_name, timestamp, hash_hex
    );

    let mut final_content = real_header.clone();
    final_content.push_str(&body);
    fs::write(&output_filename, final_content)?;

    // 6. Token statistics summing
    let mut structure_text = String::new();
    structure_text.push_str("Project Structure:\n");
    structure_text.push_str(&format!("{}/\n", root_name));
    for line in &tree_lines {
        structure_text.push_str(line);
        structure_text.push('\n');
    }
    structure_text.push('\n');

    let (struct_gpt, struct_gemini, struct_claude) = ai_counter.count_tokens_raw(&structure_text);
    let (header_gpt, header_gemini, header_claude) = ai_counter.count_tokens_raw(&real_header);

    let mut total_gpt = struct_gpt + header_gpt;
    let mut total_gemini = struct_gemini + header_gemini;
    let mut total_claude = struct_claude + header_claude;
    let mut total_words =
        structure_text.split_whitespace().count() + real_header.split_whitespace().count();
    let mut total_chars = structure_text.chars().count() + real_header.chars().count();

    for file_res in &processed_files {
        total_gpt += file_res.gpt_tokens;
        total_gemini += file_res.gemini_tokens;
        total_claude += file_res.claude_tokens;
        total_words += file_res.words;
        total_chars += file_res.chars;
    }

    let final_size = fs::metadata(&output_filename)?.len();
    println!(
        "[+] Created {} ({})",
        output_filename,
        format_file_size(final_size)
    );
    println!("[*] Files merged: {}", format_count(processed_files.len()));
    println!("[*] Files ignored: {}", format_count(total_ignored_count));

    println!("Words: {}, Characters: {}", format_count(total_words), format_count(total_chars));
    println!("SHA3-256-data: {}", hash_hex);
    println!("\nToken Statistics (there may be some margin of error): ");
    println!("GPT4-Turbo-O1-O3-Mini: ~{}", format_count(total_gpt));
    println!("Gemini-Gemma7B: ~{}", format_count(total_gemini));
    println!("Claude3.5-Sonnet-Opus: ~{}", format_count(total_claude));

    // 7. Auto-splitting logic
    let mut limit = 500_000;
    let mut always_split_if_exceeded = false;

    if let Some(ref split_opt) = options.split {
        always_split_if_exceeded = true;
        if let Some(limit_str) = split_opt {
            if let Ok(parsed) = parse_limit(limit_str) {
                limit = parsed;
            }
        }
    }

    let max_tokens = total_gpt.max(total_gemini).max(total_claude);
    let exceeded = max_tokens > limit;

    let should_split = if exceeded {
        if options.notsplit {
            false
        } else if always_split_if_exceeded {
            true
        } else {
            Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "Token count ~{} exceeds the limit of {}. Split into parts?",
                    format_count(max_tokens), format_count(limit)
                ))
                .default(true)
                .interact()
                .map_err(anyhow::Error::from)
                .or_else(crate::utils::handle_interact_error)?
        }
    } else {
        false
    };

    if should_split {
        let abs_project_dir = fs::canonicalize(&dir)?;
        let parent_dir = abs_project_dir.parent().unwrap_or(Path::new("."));
        let split_dir_name = format!("mrg-{}", root_name);
        let split_dir_path = parent_dir.join(&split_dir_name);
        fs::create_dir_all(&split_dir_path)?;

        println!("[*] Splitting project into parts in {:?}", split_dir_path);

        let mut part_num = 1;
        let mut part_files = Vec::new();
        let mut part_gpt = struct_gpt + header_gpt;
        let mut part_gemini = struct_gemini + header_gemini;
        let mut part_claude = struct_claude + header_claude;

        for file_res in &processed_files {
            let would_exceed = part_gpt + file_res.gpt_tokens > limit
                || part_gemini + file_res.gemini_tokens > limit
                || part_claude + file_res.claude_tokens > limit;

            if would_exceed && !part_files.is_empty() {
                write_part(
                    &split_dir_path,
                    &root_name,
                    part_num,
                    version,
                    &timestamp,
                    &tree_lines,
                    &part_files,
                )?;
                part_num += 1;
                part_files.clear();
                part_gpt = struct_gpt + header_gpt;
                part_gemini = struct_gemini + header_gemini;
                part_claude = struct_claude + header_claude;
            }

            part_files.push(file_res);
            part_gpt += file_res.gpt_tokens;
            part_gemini += file_res.gemini_tokens;
            part_claude += file_res.claude_tokens;
        }

        if !part_files.is_empty() {
            write_part(
                &split_dir_path,
                &root_name,
                part_num,
                version,
                &timestamp,
                &tree_lines,
                &part_files,
            )?;
        }
    }

    Ok(())
}

fn write_part(
    split_dir_path: &Path,
    root_name: &str,
    part_num: usize,
    version: &str,
    timestamp: &str,
    tree_lines: &[String],
    part_files: &[&FileResult],
) -> Result<()> {
    let part_filename = format!("mrg-{}-part{}.txt", root_name, part_num);
    let part_path = split_dir_path.join(&part_filename);

    let mut body = String::new();
    body.push_str("Project Structure:\n");
    body.push_str(&format!("{}/\n", root_name));
    for line in tree_lines {
        body.push_str(line);
        body.push('\n');
    }
    body.push('\n');

    for file_res in part_files {
        body.push_str(&format!("=== start {} ===\n", file_res.rel_path));
        body.push_str(&file_res.content);
        if !file_res.content.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&format!("=== end {} ===\n\n", file_res.rel_path));
    }

    let hash_hex = calculate_sha3_256(&body);

    let mut content = format!(
        "Project merger tool v{}\n{} (part {}) ({})\nhash(sha3-256):{}\n**********\n",
        version, root_name, part_num, timestamp, hash_hex
    );
    content.push_str(&body);

    fs::write(&part_path, content)?;
    println!(
        "[+] Created split part: {} ({})",
        part_filename,
        format_file_size(fs::metadata(&part_path)?.len())
    );
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

pub fn run_tokenize(file_path: Option<PathBuf>) -> Result<()> {
    let target_path = match file_path {
        Some(path) => path,
        None => select_mrg_file()?,
    };

    if !target_path.exists() {
        anyhow::bail!("Error: File {:?} does not exist.", target_path);
    }

    if is_binary_file(&target_path)? {
        anyhow::bail!("Error: Cannot tokenize binary file.");
    }

    let file_content = fs::read_to_string(&target_path)?;
    println!("[*] Tokenizing file: {:?}", target_path);

    // Initialize all 10 tokenizers
    let ai_counter = AICounter::new("AItokenizers", true).map_err(|e| anyhow::anyhow!("{}", e))?;

    let counts = ai_counter.count_tokens_all(&file_content);

    println!("\nToken Statistics for all models:");
    
    let display_order = [
        (crate::tokenizer::ModelKind::Gpt4oO1O3Mini, "GPT4-Turbo-O1-O3-Mini:"),
        (crate::tokenizer::ModelKind::Gpt4TurboGpt35Turbo, "GPT4-Turbo-GPT3.5-Turbo:"),
        (crate::tokenizer::ModelKind::GeminiGemma7b, "Gemini-Gemma7B:"),
        (crate::tokenizer::ModelKind::Claude35SonnetOpus, "Claude3.5-Sonnet-Opus:"),
        (crate::tokenizer::ModelKind::Llama32, "LLAMA3-3.1-3.2:"),
        (crate::tokenizer::ModelKind::DeepSeekV2V3R1, "DeepSeekV2-V3-R1:"),
        (crate::tokenizer::ModelKind::Qwen25Coder, "Qwen2.5-Coder:"),
        (crate::tokenizer::ModelKind::MistralCodestral, "Mistral-Codestral:"),
        (crate::tokenizer::ModelKind::Phi3Phi4, "Phi3-Phi4:"),
        (crate::tokenizer::ModelKind::CohereCommandRPlus, "Cohere-CommandR-R+:"),
    ];

    for &(kind, label) in &display_order {
        let count = counts.get(&kind).copied().unwrap_or(0);
        println!("{} ~{}", label, format_count(count));
    }

    Ok(())
}
