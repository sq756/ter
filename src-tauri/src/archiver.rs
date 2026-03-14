use std::path::PathBuf;
use std::fs;
use std::io::Write;
use regex::Regex;
use chrono::Local;
use lazy_static::lazy_static;

pub struct GhostArchiver {
    archives_dir: PathBuf,
    ansi_regex: Regex,
    // v2.11.37: Semantic anchors for Agent's thought chain
    initiation_regex: Regex,
    action_keyword_regex: Regex,
}

impl GhostArchiver {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let archives_dir = PathBuf::from(home).join(".ter/archives");
        if !archives_dir.exists() {
            let _ = fs::create_dir_all(&archives_dir);
        }
        Self {
            archives_dir,
            ansi_regex: Regex::new(r"\x1B\[([0-9;]*[a-zA-Z])").unwrap(),
            initiation_regex: Regex::new(r"(?i)(I will|Then, I'll|Finally, I'll)").unwrap(),
            action_keyword_regex: Regex::new(r"(?i)(I will|Then, I'll|Finally, I'll)\s+(\w+)").unwrap(),
        }
    }

    pub fn strip_ansi(&self, data: &[u8]) -> String {
        let s = String::from_utf8_lossy(data);
        self.ansi_regex.replace_all(&s, "").to_string()
    }

    pub fn extract_summary(&self, text: &str) -> String {
        if let Some(caps) = self.action_keyword_regex.captures(text) {
            if let Some(word) = caps.get(2) {
                return word.as_str().to_uppercase();
            }
        }
        "AI_INTERACTION".to_string()
    }

    pub fn archive(&self, tab_id: &str, data: &[u8]) {
        let clean_text = self.strip_ansi(data);
        if clean_text.trim().is_empty() { return; }

        let timestamp_file = Local::now().format("%Y%m%d_%H%M%S");
        let summary = self.extract_summary(&clean_text);
        
        // Create both a specific entry and update the 'latest' link
        let file_name = format!("{}_{}_{}.md", timestamp_file, tab_id, summary);
        let file_path = self.archives_dir.join(&file_name);
        let latest_path = self.archives_dir.join(format!("{}_latest.md", tab_id));

        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&file_path) {
            let _ = file.write_all(clean_text.as_bytes());
        }
        
        // v2.11.37: Update latest file for CLIP button
        let _ = fs::write(latest_path, clean_text);
    }

    pub fn is_semantic_start(&self, data: &[u8]) -> bool {
        let clean = self.strip_ansi(data);
        self.initiation_regex.is_match(&clean)
    }

    pub fn get_latest(&self, tab_id: &str) -> Result<String, String> {
        let file_path = self.archives_dir.join(format!("{}_latest.md", tab_id));
        if !file_path.exists() {
            return Err("ERROR: ARCHIVE_NOT_FOUND. Initiate dialogue with 'I will...'".to_string());
        }
        fs::read_to_string(file_path).map_err(|e| e.to_string())
    }

    pub fn list_vault(&self) -> Result<Vec<serde_json::Value>, String> {
        let mut entries = Vec::new();
        if let Ok(paths) = fs::read_dir(&self.archives_dir) {
            for path in paths.flatten() {
                let name = path.file_name().to_string_lossy().into_owned();
                if name.ends_with(".md") && !name.contains("latest") {
                    entries.push(serde_json::json!({
                        "name": name,
                        "path": path.path().to_string_lossy()
                    }));
                }
            }
        }
        entries.sort_by(|a, b| b["name"].as_str().unwrap().cmp(a["name"].as_str().unwrap()));
        Ok(entries)
    }
}

lazy_static! {
    pub static ref ARCHIVER: GhostArchiver = GhostArchiver::new();
}
