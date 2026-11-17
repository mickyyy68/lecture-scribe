//! Parakeet V3 0.6B (25 European languages), the one speech model the app uses: finding it
//! (the app's own download, Handy's folder or the Hugging Face cache) and downloading it.

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
    let mut roots = vec![models_dir()];
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

/// Where the app downloads the model.
pub fn models_dir() -> PathBuf {
    crate::paths::data_dir().join("models")
}

/// The download: NVIDIA's Parakeet TDT 0.6B V3 (CC-BY-4.0) as GGUF, 8-bit, ~740 MB.
pub const DOWNLOAD_FILE: &str = "parakeet-tdt-0.6b-v3-Q8_0.gguf";
const DOWNLOAD_URL: &str = "https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf/resolve/main/parakeet-tdt-0.6b-v3-Q8_0.gguf";
const DOWNLOAD_SHA256: &str = "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7";
pub const DOWNLOAD_BYTES: u64 = 739_508_576;

/// The Parakeet V3 model on this Mac, if any: the 8-bit one first, then other quantizations.
pub fn discover() -> Vec<ModelInfo> {
    let mut files = Vec::new();
    for r in search_roots() {
        walk(&r, 5, &mut files);
    }
    let rank = |q: &str| ["Q8_0", "F16", "Q6_K", "Q5_K_M", "F32", "Q4_K_M"].iter().position(|x| *x == q).unwrap_or(9);
    let mut found: Vec<(usize, ModelInfo)> = files
        .into_iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if !(lower.contains("parakeet") && lower.contains("v3")) || name.ends_with(".part") {
                return None;
            }
            let quant = name.trim_end_matches(".gguf").rsplit('-').next().unwrap_or("").to_string();
            let label = format!("Parakeet V3 0.6B (25 European languages) · {quant}");
            Some((rank(&quant), ModelInfo { path: p.to_string_lossy().to_string(), label, langs: "multi" }))
        })
        .collect();
    found.sort_by_key(|(r, m)| (*r, m.path.clone()));
    found.into_iter().take(1).map(|(_, m)| m).collect()
}

/// Downloads the model into `models_dir()` with curl, calling `progress(bytes so far)` about
/// every half second, and checks its SHA-256 before keeping it.
pub fn download(progress: &dyn Fn(u64)) -> Result<PathBuf, String> {
    let dir = models_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let (part, dest) = (dir.join(format!("{DOWNLOAD_FILE}.part")), dir.join(DOWNLOAD_FILE));
    // -C - resumes an interrupted download.
    let mut child = std::process::Command::new("curl")
        .args(["-L", "--fail", "-sS", "-C", "-", "--retry", "3", "-o"])
        .arg(&part)
        .arg(DOWNLOAD_URL)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Can't start the download: {e}"))?;
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st;
        }
        progress(std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0));
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
    if !status.success() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut e, &mut err);
        }
        return Err(format!("The download failed: {}", err.trim()));
    }
    progress(std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0));
    let out = std::process::Command::new("shasum").args(["-a", "256"]).arg(&part).output().map_err(|e| e.to_string())?;
    let sum = String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap_or("").to_string();
    if sum != DOWNLOAD_SHA256 {
        let _ = std::fs::remove_file(&part);
        return Err("The downloaded model is damaged (checksum mismatch). Try again.".into());
    }
    std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}
