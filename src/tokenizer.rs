use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use tokenizers::tokenizer::Tokenizer;

#[allow(dead_code)]
pub struct AnalysisResult {
    pub words: usize,
    pub chars: usize,
    pub gpt_string: String,
    pub gemini_string: String,
    pub claude_string: String,
}

pub struct AICounter {
    gpt: Tokenizer,
    gemini: Tokenizer,
    claude: Tokenizer,
}

impl AICounter {
    pub fn new(folder_name: &str) -> Result<Self, Box<dyn Error>> {
        let exe_path = std::env::current_exe()?;
        let exe_dir = exe_path.parent().ok_or("Root dir not found")?;
        let tokenizers_dir = exe_dir.join(folder_name);

        if !tokenizers_dir.exists() {
            fs::create_dir_all(&tokenizers_dir)?;
        }

        let gpt = Self::load_or_download(
            "GPT-4o",
            "https://raw.githubusercontent.com/cyberuser0x33/mrgfilesAiTokenizers/main/tokenizers/gpt.json",
            &tokenizers_dir.join("gpt.json"),
        )?;

        let gemini = Self::load_or_download(
            "Gemini",
            "https://raw.githubusercontent.com/cyberuser0x33/mrgfilesAiTokenizers/main/tokenizers/gemini.json",
            &tokenizers_dir.join("gemini.json"),
        )?;

        let claude = Self::load_or_download(
            "Claude",
            "https://raw.githubusercontent.com/cyberuser0x33/mrgfilesAiTokenizers/main/tokenizers/claude.json",
            &tokenizers_dir.join("claude.json"),
        )?;

        Ok(Self {
            gpt,
            gemini,
            claude,
        })
    }

    fn load_or_download(
        name: &str,
        url: &str,
        path: &PathBuf,
    ) -> Result<Tokenizer, Box<dyn Error>> {
        if !path.exists() {
            println!("[*] Downloading tokenizer for {}...", name);

            let client = Client::builder().build()?;
            let response = client
                .get(url)
                .header(USER_AGENT, "rust-tokenizer-app/1.0")
                .send()?;

            let status = response.status();

            if !status.is_success() {
                let err_text = response.text().unwrap_or_else(|_| "Unknown error".into());
                return Err(
                    format!("Server error {}: {}. Response: {}", name, status, err_text).into(),
                );
            }

            let bytes = response.bytes()?;
            fs::write(path, bytes)?;
            println!("[+] Successfully downloaded {}", name);
        }

        let t = Tokenizer::from_file(path).map_err(|e| {
            if let Ok(content) = fs::read_to_string(path) {
                println!("--- DEBUG INFO ---");
                println!(
                    "File {} content starts with: {}",
                    name,
                    content.chars().take(100).collect::<String>()
                );
                println!("------------------");
            }
            let _ = fs::remove_file(path);
            format!(
                "Dictionary error {}: {}. File was deleted, please try again.",
                name, e
            )
        })?;

        Ok(t)
    }

    fn count_tokens_chunked(tokenizer: &Tokenizer, text: &str) -> usize {
        const CHUNK_SIZE: usize = 64 * 1024; // 64 KB
        let mut total = 0;
        let mut start = 0;
        let bytes = text.as_bytes();
        while start < bytes.len() {
            let mut end = start + CHUNK_SIZE;
            if end >= bytes.len() {
                end = bytes.len();
            } else {
                while end > start && !text.is_char_boundary(end) {
                    end -= 1;
                }
                if end == start {
                    end = start + CHUNK_SIZE;
                    while end < bytes.len() && !text.is_char_boundary(end) {
                        end += 1;
                    }
                }
            }
            let chunk = &text[start..end];
            if let Ok(encoding) = tokenizer.encode(chunk, true) {
                total += encoding.get_ids().len();
            }
            start = end;
        }
        total
    }

    pub fn count_tokens_raw(&self, text: &str) -> (usize, usize, usize) {
        let gpt = Self::count_tokens_chunked(&self.gpt, text);
        let gemini = Self::count_tokens_chunked(&self.gemini, text);
        let claude = Self::count_tokens_chunked(&self.claude, text);
        (gpt, gemini, claude)
    }

    #[allow(dead_code)]
    pub fn count_string(&self, text: &str) -> AnalysisResult {
        let words = text.split_whitespace().count();
        let chars = text.chars().count();

        let gpt_count = Self::count_tokens_chunked(&self.gpt, text);
        let gemini_count = Self::count_tokens_chunked(&self.gemini, text);
        let claude_count = Self::count_tokens_chunked(&self.claude, text);

        AnalysisResult {
            words,
            chars,
            gpt_string: format!("GPT-models: ~{}", gpt_count),
            gemini_string: format!("Gemini-models: ~{}", gemini_count),
            claude_string: format!("Claude-models: ~{}", claude_count),
        }
    }
}
