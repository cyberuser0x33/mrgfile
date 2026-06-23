use anyhow::Result;
use chrono::Utc;
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use regex::Regex;
use sha3::{Digest, Sha3_256};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingMode {
    Full,
    Minify,
    Maximize,
}

#[derive(Debug, Clone, Default)]
pub struct MaximizeFilters {
    pub directories: Vec<String>,
    pub files: Vec<String>,
}

impl MaximizeFilters {
    pub fn parse(args: &[String]) -> Self {
        let mut directories = Vec::new();
        let mut files = Vec::new();
        for arg in args {
            let arg_clean = arg.trim_matches('"').trim_matches('\'');
            if arg_clean.starts_with("d=") {
                let val = arg_clean["d=".len()..].trim_matches('"').trim_matches('\'');
                directories.push(val.to_string());
            } else if arg_clean.starts_with("f=") {
                let val = arg_clean["f=".len()..].trim_matches('"').trim_matches('\'');
                files.push(val.to_string());
            }
        }
        Self { directories, files }
    }

    pub fn matches(&self, rel_path: &str) -> bool {
        if self.directories.is_empty() && self.files.is_empty() {
            return true;
        }
        let path = Path::new(rel_path);
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if self.files.iter().any(|f| f == file_name) {
                return true;
            }
        }
        for component in path.components() {
            if let Some(comp_str) = component.as_os_str().to_str() {
                if self.directories.iter().any(|d| d == comp_str) {
                    return true;
                }
            }
        }
        false
    }
}

pub fn get_skeleton_note(extension: &str) -> &'static str {
    match extension {
        "py" | "sh" | "yaml" | "yml" | "toml" | "rb" | "r" => {
            "# [NOTE: This is a structural skeleton, body removed for brevity]\n"
        }
        "html" | "xml" => {
            "<!-- [NOTE: This is a structural skeleton, body removed for brevity] -->\n"
        }
        _ => "// [NOTE: This is a structural skeleton, body removed for brevity]\n",
    }
}

pub fn minify_content(content: &str, extension: &str) -> String {
    let mut minified = String::new();
    let mut in_multiline_comment = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut line_to_add = line.to_string();

        match extension {
            "rs" | "js" | "ts" | "go" | "java" | "cpp" | "c" | "h" | "cs" | "css" => {
                if in_multiline_comment {
                    if let Some(pos) = line_to_add.find("*/") {
                        in_multiline_comment = false;
                        line_to_add = line_to_add[pos + 2..].to_string();
                    } else {
                        continue;
                    }
                }

                while let Some(start_pos) = line_to_add.find("/*") {
                    if let Some(end_pos) = line_to_add[start_pos..].find("*/") {
                        let actual_end = start_pos + end_pos + 2;
                        line_to_add = format!(
                            "{}{}",
                            &line_to_add[..start_pos],
                            &line_to_add[actual_end..]
                        );
                    } else {
                        line_to_add = line_to_add[..start_pos].to_string();
                        in_multiline_comment = true;
                        break;
                    }
                }

                if let Some(pos) = line_to_add.find("//") {
                    if pos == 0 || (pos > 0 && &line_to_add[pos - 1..pos] != ":") {
                        line_to_add = line_to_add[..pos].to_string();
                    }
                }
            }
            "py" | "sh" | "yaml" | "yml" | "toml" | "rb" | "r" => {
                if let Some(pos) = line_to_add.find('#') {
                    line_to_add = line_to_add[..pos].to_string();
                }
            }
            "html" | "xml" => {
                if in_multiline_comment {
                    if let Some(pos) = line_to_add.find("-->") {
                        in_multiline_comment = false;
                        line_to_add = line_to_add[pos + 3..].to_string();
                    } else {
                        continue;
                    }
                }
                while let Some(start_pos) = line_to_add.find("<!--") {
                    if let Some(end_pos) = line_to_add[start_pos..].find("-->") {
                        let actual_end = start_pos + end_pos + 3;
                        line_to_add = format!(
                            "{}{}",
                            &line_to_add[..start_pos],
                            &line_to_add[actual_end..]
                        );
                    } else {
                        line_to_add = line_to_add[..start_pos].to_string();
                        in_multiline_comment = true;
                        break;
                    }
                }
            }
            _ => {}
        }

        let final_line = line_to_add.trim();
        if !final_line.is_empty() {
            // Re-trim end of the line, keeping leading indentation
            let leading_indent_len = line_to_add.len() - line_to_add.trim_start().len();
            let indent = &line_to_add[..leading_indent_len];
            minified.push_str(indent);
            minified.push_str(final_line);
            minified.push('\n');
        }
    }

    minified
}

thread_local! {
    static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

pub fn maximize_content(content: &str, extension: &str) -> String {
    let mut skeleton = String::new();
    skeleton.push_str(get_skeleton_note(extension));

    let lang = match extension {
        "rs" => Some(tree_sitter_rust::language()),
        "py" => Some(tree_sitter_python::language()),
        "js" | "jsx" | "ts" | "tsx" => Some(tree_sitter_javascript::language()),
        "go" => Some(tree_sitter_go::language()),
        _ => None,
    };

    if let Some(l) = lang {
        let parsed_output = THREAD_PARSER.with(|parser_cell| {
            let mut parser = parser_cell.borrow_mut();
            if parser.set_language(l).is_ok() {
                if let Some(tree) = parser.parse(content, None) {
                    let source_bytes = content.as_bytes();
                    let root = tree.root_node();
                    let mut output = String::new();
                    traverse_node(root, source_bytes, extension, 0, &mut output);
                    if !output.trim().is_empty() {
                        return Some(output);
                    }
                }
            }
            None
        });
        if let Some(output) = parsed_output {
            skeleton.push_str(&output);
            return skeleton;
        }
    }

    let fallback = maximize_content_fallback(content, extension);
    skeleton.push_str(&fallback);
    skeleton
}

fn traverse_node<'a>(
    node: Node<'a>,
    source: &'a [u8],
    extension: &str,
    depth: usize,
    output: &mut String,
) {
    let kind = node.kind();

    let is_class_container = match extension {
        "rs" => kind == "impl_item" || kind == "trait_item" || kind == "mod_item",
        "py" => kind == "class_definition",
        "js" | "jsx" | "ts" | "tsx" => {
            kind == "class_declaration" || kind == "class" || kind == "interface_declaration"
        }
        _ => false,
    };

    let is_function = match extension {
        "rs" => kind == "function_item",
        "py" => kind == "function_definition",
        "js" | "jsx" | "ts" | "tsx" => {
            kind == "function_declaration" || kind == "method_definition"
        }
        "go" => kind == "function_declaration" || kind == "method_declaration",
        _ => false,
    };

    let is_standalone_type = match extension {
        "rs" => kind == "struct_item" || kind == "enum_item",
        "go" => kind == "type_declaration",
        _ => false,
    };

    if is_class_container {
        let body_kind = match extension {
            "rs" => "declaration_list",
            "py" => "block",
            "js" | "jsx" | "ts" | "tsx" => "class_body",
            _ => "declaration_list",
        };

        let body_child = find_child_by_kind(node, body_kind);
        if let Some(bc) = body_child {
            let sig = extract_signature(node, bc, source);
            output.push_str(&sig);
            if extension == "py" {
                output.push_str(":\n");
            } else {
                output.push_str(" {\n");
            }

            let mut cursor = bc.walk();
            for child in bc.children(&mut cursor) {
                traverse_node(child, source, extension, depth + 1, output);
            }

            if extension != "py" {
                let indent = get_indent(node, source);
                output.push_str(&indent);
                output.push_str("}\n\n");
            }
        } else {
            let text = get_node_text(node, source);
            output.push_str(text);
            output.push('\n');
        }
    } else if is_function {
        let body_kind = match extension {
            "rs" => "block",
            "py" => "block",
            "js" | "jsx" | "ts" | "tsx" => "statement_block",
            "go" => "block",
            _ => "block",
        };

        let body_child = find_child_by_kind(node, body_kind);
        if let Some(bc) = body_child {
            let sig = extract_signature(node, bc, source);
            output.push_str(&sig);
            if extension == "py" {
                output.push_str(" ...\n");
            } else {
                output.push_str(" { ... }\n");
            }
        } else {
            let text = get_node_text(node, source);
            output.push_str(text);
            output.push('\n');
        }
    } else if is_standalone_type {
        let body_kind = match extension {
            "rs" => "field_declaration_list",
            "go" => "struct_type",
            _ => "field_declaration_list",
        };

        let mut body_child = find_child_by_kind(node, body_kind);
        if body_child.is_none() && extension == "rs" {
            body_child = find_child_by_kind(node, "enum_variant_list");
        }
        if body_child.is_none() && extension == "go" {
            body_child = find_nested_child_by_kinds(node, &["type_spec", "struct_type"]);
            if body_child.is_none() {
                body_child = find_nested_child_by_kinds(node, &["type_spec", "interface_type"]);
            }
        }

        if let Some(bc) = body_child {
            let sig = extract_signature(node, bc, source);
            output.push_str(&sig);
            output.push_str(" { ... }\n");
        } else {
            let text = get_node_text(node, source);
            output.push_str(text);
            output.push('\n');
        }
    } else {
        if node.parent().is_none()
            || kind == "source_file"
            || kind == "module"
            || kind == "translation_unit"
        {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                traverse_node(child, source, extension, depth, output);
            }
        }
    }
}

fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return Some(child);
        }
    }
    None
}

fn find_nested_child_by_kinds<'a>(node: Node<'a>, kinds: &[&str]) -> Option<Node<'a>> {
    if kinds.is_empty() {
        return Some(node);
    }
    let target = kinds[0];
    let rest = &kinds[1..];

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == target {
            return find_nested_child_by_kinds(child, rest);
        }
    }
    None
}

fn get_node_text<'a>(node: Node<'a>, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("")
}

fn get_indent(node: Node<'_>, source: &[u8]) -> String {
    let start_byte = node.start_byte();
    let mut line_start = start_byte;
    while line_start > 0 && source[line_start - 1] != b'\n' {
        line_start -= 1;
    }
    let indent_bytes = &source[line_start..start_byte];
    let mut indent = String::new();
    for &b in indent_bytes {
        if b == b' ' || b == b'\t' {
            indent.push(b as char);
        } else {
            break;
        }
    }
    indent
}

fn extract_signature(node: Node<'_>, body_node: Node<'_>, source: &[u8]) -> String {
    let start_byte = node.start_byte();
    let mut line_start = start_byte;
    while line_start > 0 && source[line_start - 1] != b'\n' {
        line_start -= 1;
    }
    let sig_bytes = &source[line_start..body_node.start_byte()];
    let sig_str = std::str::from_utf8(sig_bytes).unwrap_or("").trim_end();
    sig_str.to_string()
}

pub fn maximize_content_fallback(content: &str, extension: &str) -> String {
    static RS_REGEX: OnceLock<Regex> = OnceLock::new();
    static PY_REGEX: OnceLock<Regex> = OnceLock::new();
    static JS_TS_REGEX: OnceLock<Regex> = OnceLock::new();
    static GO_REGEX: OnceLock<Regex> = OnceLock::new();
    static CPP_CS_JV_REGEX: OnceLock<Regex> = OnceLock::new();
    static DEFAULT_REGEX: OnceLock<Regex> = OnceLock::new();

    let regex = match extension {
        "rs" => RS_REGEX.get_or_init(|| {
            Regex::new(r#"^\s*(pub(\([^)]+\))?\s+)?(async\s+)?(const\s+)?(unsafe\s+)?(extern\s+("[^"]+"\s+)?)?(fn|struct|enum|impl|trait)\b"#).unwrap()
        }),
        "py" => PY_REGEX.get_or_init(|| {
            Regex::new(r#"^\s*(def|class)\b"#).unwrap()
        }),
        "js" | "ts" | "jsx" | "tsx" => JS_TS_REGEX.get_or_init(|| {
            Regex::new(r#"^\s*(export\s+(default\s+)?)?(async\s+)?(function|class|interface|type)\b"#).unwrap()
        }),
        "go" => GO_REGEX.get_or_init(|| {
            Regex::new(r#"^\s*(func|type)\b"#).unwrap()
        }),
        "java" | "cs" | "cpp" | "h" | "hpp" => CPP_CS_JV_REGEX.get_or_init(|| {
            Regex::new(r#"\b(class|struct|interface|enum)\b|(\b(void|int|string|bool|float|double|public|private|protected|static|virtual|override|async)\b.*\()"#).unwrap()
        }),
        _ => DEFAULT_REGEX.get_or_init(|| {
            Regex::new(r#"^\s*(def|class|fn|func|struct)\b"#).unwrap()
        }),
    };

    let mut skeleton = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if regex.is_match(trimmed) {
            let indent_len = line.len() - line.trim_start().len();
            let indent = &line[..indent_len];
            skeleton.push_str(indent);
            skeleton.push_str(trimmed);
            skeleton.push('\n');
        }
    }
    skeleton
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub is_dir: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new(name: String, is_dir: bool) -> Self {
        Self {
            name,
            is_dir,
            children: Vec::new(),
        }
    }

    pub fn insert(&mut self, path: &Path, is_dir: bool) {
        let mut current = self;
        for component in path.components() {
            let name = component.as_os_str().to_string_lossy().into_owned();
            if name.is_empty() || name == "." {
                continue;
            }
            let pos = current.children.iter().position(|c| c.name == name);
            if let Some(p) = pos {
                current = &mut current.children[p];
            } else {
                let is_last = component == path.components().last().unwrap();
                let new_node = TreeNode::new(name, if is_last { is_dir } else { true });
                current.children.push(new_node);
                let len = current.children.len();
                current = &mut current.children[len - 1];
            }
        }
    }

    pub fn sort(&mut self) {
        self.children.sort_by(|a, b| {
            if a.is_dir != b.is_dir {
                b.is_dir.cmp(&a.is_dir)
            } else {
                a.name.cmp(&b.name)
            }
        });
        for child in &mut self.children {
            child.sort();
        }
    }

    pub fn build_lines(&self, prefix: &str, lines: &mut Vec<String>) {
        let count = self.children.len();
        for (i, child) in self.children.iter().enumerate() {
            let is_last = i == count - 1;
            let connector = if is_last { "└── " } else { "├── " };
            lines.push(format!("{}{}{}", prefix, connector, child.name));
            if child.is_dir {
                let next_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                child.build_lines(&next_prefix, lines);
            }
        }
    }
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

    let items: Vec<String> = dirs
        .iter()
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a project directory:")
        .items(&items)
        .default(0)
        .interact()
        .map_err(anyhow::Error::from)
        .or_else(handle_interact_error)?;

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

    files.sort_by_key(|p| {
        fs::metadata(p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::now())
    });
    files.reverse();

    if files.len() == 1 {
        return Ok(files[0].clone());
    }

    let items: Vec<String> = files
        .iter()
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select an mrg file:")
        .items(&items)
        .default(0)
        .interact()
        .map_err(anyhow::Error::from)
        .or_else(handle_interact_error)?;

    Ok(files[selection].clone())
}

pub fn handle_interact_error<T>(err: anyhow::Error) -> Result<T> {
    println!();
    let exit_confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Finish the process?")
        .default(true)
        .interact();
    match exit_confirm {
        Ok(true) => {
            println!("[*] Operation cancelled by user. Exiting cleanly.");
            std::process::exit(0);
        }
        _ => Err(err),
    }
}

pub fn is_binary_file<P: AsRef<Path>>(path: P) -> Result<bool> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut buffer = [0u8; 8192];
    let bytes_read = file.read(&mut buffer)?;
    for &b in &buffer[..bytes_read] {
        if b == 0 {
            return Ok(true);
        }
    }
    Ok(false)
}
