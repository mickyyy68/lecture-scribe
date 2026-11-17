//! Where the app keeps its files.

use serde_json::{json, Value};
use std::path::PathBuf;

pub const APP_NAME: &str = "Transcribe Lecture";
/// The app's name before it was open-sourced. Its data folder is used when it exists and the new
/// one doesn't, so glossaries, course PDFs and settings carry over.
const LEGACY_NAME: &str = "Lecture Scribe";

/// `~/Library/Application Support/Transcribe Lecture`: settings, themes, glossaries, course PDFs.
pub fn data_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    let (new, old) = (base.join(APP_NAME), base.join(LEGACY_NAME));
    if !new.exists() && old.is_dir() { old } else { new }
}

/// `~/Documents/Lecture Transcripts`: transcripts, audio and decks.
pub fn out_dir() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_else(std::env::temp_dir)).join("Lecture Transcripts")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn load_settings() -> Value {
    std::fs::read_to_string(settings_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}))
}
