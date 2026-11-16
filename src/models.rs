//! Finds the Parakeet GGUF models Handy has downloaded.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug)]
pub struct ModelInfo {
    pub path: String,
    pub label: String,
    /// "en" for English-only models, "multi" for multilingual.
    pub langs: &'static str,
}

fn search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".cache/huggingface/hub"));
        roots.push(home.join("Library/Application Support/com.pais.handy/models"));
    }
    if let Ok(hf) = std::env::var("HF_HOME") {
        roots.push(PathBuf::from(hf).join("hub"));
    }
    roots
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, depth - 1, out);
        } else if p.extension().is_some_and(|x| x == "gguf") {
            out.push(p);
        }
    }
}

pub fn discover() -> Vec<ModelInfo> {
    let mut files = Vec::new();
    for r in search_roots() {
        walk(&r, 5, &mut files);
    }
    let mut models: Vec<ModelInfo> = files
        .into_iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if !lower.contains("parakeet") {
                return None;
            }
            let quant = name
                .trim_end_matches(".gguf")
                .rsplit('-')
                .next()
                .unwrap_or("")
                .to_string();
            let (label, langs) = if lower.contains("unified") {
                ("Parakeet Unified 0.6B (English)".to_string(), "en")
            } else if lower.contains("v3") {
                ("Parakeet V3 0.6B (25 languages incl. Italian)".to_string(), "multi")
            } else if lower.contains("v2") {
                ("Parakeet V2 0.6B (English)".to_string(), "en")
            } else {
                (name.clone(), "en")
            };
            Some(ModelInfo { path: p.to_string_lossy().to_string(), label: format!("{label} · {quant}"), langs })
        })
        .collect();
    // Parakeet V3 (Italian) first: it is the default.
    models.sort_by_key(|m| (!m.label.contains("V3"), m.label.clone()));
    models.dedup_by(|a, b| a.label == b.label);
    models
}
