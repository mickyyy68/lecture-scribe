//! Per-course glossary that DeepSeek fills in when it makes slides.
//!
//! `corrections` are misrecognitions ("HCB" → "DHCP") that the engine fixes live in new
//! transcripts; `terms` are the course's key terms, sent back to DeepSeek as context. Parakeet
//! has no vocabulary biasing, so this is post-correction, like Handy's custom words.
//!
//! Stored in ~/Library/Application Support/Lecture Scribe/glossary/<course>.json

use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::SystemTime;

/// File-name-safe form of a course or file name.
pub fn slug(course: &str) -> String {
    let slug: String = course
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() { "generale".to_string() } else { slug }
}

pub fn path(course: &str) -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("Lecture Scribe").join("glossary").join(format!("{}.json", slug(course)))
}

pub fn load(course: &str) -> Value {
    let mut v: Value = std::fs::read_to_string(path(course)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
    for k in ["corrections", "terms"] {
        if !v[k].is_array() {
            v[k] = json!([]);
        }
    }
    v
}

fn save(course: &str, v: &Value) -> Result<(), String> {
    let p = path(course);
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(v).unwrap()).and_then(|_| std::fs::rename(&tmp, &p)).map_err(|e| e.to_string())
}

/// Adds DeepSeek's suggestions; returns how many corrections and terms were new.
pub fn merge(course: &str, suggested: &Value) -> (usize, usize) {
    let mut g = load(course);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let (mut nc, mut nt) = (0, 0);
    for c in suggested["corrections"].as_array().into_iter().flatten() {
        let heard = c["heard"].as_str().unwrap_or("").trim();
        let correct = c["correct"].as_str().unwrap_or("").trim();
        let ok = heard.chars().count() >= 3
            && heard.chars().count() <= 40
            && !correct.is_empty()
            && !heard.contains('\n')
            && heard.to_lowercase() != correct.to_lowercase();
        let known = g["corrections"].as_array().unwrap().iter().any(|x| x["heard"].as_str().unwrap_or("").to_lowercase() == heard.to_lowercase());
        if ok && !known {
            g["corrections"].as_array_mut().unwrap().push(json!({ "heard": heard, "correct": correct, "added": today }));
            nc += 1;
        }
    }
    for t in suggested["terms"].as_array().into_iter().flatten() {
        let t = t.as_str().unwrap_or("").trim();
        let known = g["terms"].as_array().unwrap().iter().any(|x| x.as_str().unwrap_or("").to_lowercase() == t.to_lowercase());
        if !t.is_empty() && t.chars().count() <= 60 && !known {
            g["terms"].as_array_mut().unwrap().push(json!(t));
            nt += 1;
        }
    }
    if nc + nt > 0 {
        let _ = save(course, &g);
    }
    (nc, nt)
}

/// Removes a correction (by its "heard" form) or a term.
pub fn remove(course: &str, kind: &str, value: &str) -> Result<Value, String> {
    let mut g = load(course);
    let key = value.to_lowercase();
    if kind == "term" {
        g["terms"].as_array_mut().unwrap().retain(|x| x.as_str().unwrap_or("").to_lowercase() != key);
    } else {
        g["corrections"].as_array_mut().unwrap().retain(|x| x["heard"].as_str().unwrap_or("").to_lowercase() != key);
    }
    save(course, &g)?;
    Ok(g)
}

/// Applies the corrections to finished text. Reloads the file when it changes, so entries
/// learned or removed during a lecture take effect on the next sentence.
pub struct Corrector {
    path: PathBuf,
    stamp: Option<SystemTime>,
    pairs: Vec<(Vec<char>, String)>,
}

impl Corrector {
    pub fn new(course: &str) -> Self {
        let mut c = Corrector { path: path(course), stamp: None, pairs: Vec::new() };
        c.reload();
        c
    }

    fn reload(&mut self) {
        let stamp = std::fs::metadata(&self.path).and_then(|m| m.modified()).ok();
        if stamp == self.stamp && stamp.is_some() {
            return;
        }
        self.stamp = stamp;
        let v: Value = std::fs::read_to_string(&self.path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
        self.pairs = v["corrections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| Some((lower(c["heard"].as_str()?), c["correct"].as_str()?.to_string())))
            .filter(|(h, _)| h.len() >= 3)
            .collect();
        self.pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len())); // longest first
    }

    pub fn apply(&mut self, text: &str) -> String {
        self.reload();
        if self.pairs.is_empty() {
            return text.to_string();
        }
        let orig: Vec<char> = text.chars().collect();
        let low = lower(text);
        let word = |c: Option<&char>| c.is_some_and(|c| c.is_alphanumeric());
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        'outer: while i < orig.len() {
            if i == 0 || !word(orig.get(i - 1)) {
                for (h, fix) in &self.pairs {
                    let end = i + h.len();
                    if end <= low.len() && low[i..end] == h[..] && !word(orig.get(end)) {
                        out.push_str(fix);
                        i = end;
                        continue 'outer;
                    }
                }
            }
            out.push(orig[i]);
            i += 1;
        }
        out
    }
}

/// One lowercase char per input char, so indexes line up with the original.
fn lower(s: &str) -> Vec<char> {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_words_only() {
        let p = std::env::temp_dir().join("ls-glossary-test.json");
        std::fs::write(&p, r#"{"corrections":[{"heard":"HCB","correct":"DHCP"},{"heard":"Ruter","correct":"router"}]}"#).unwrap();
        let mut c = Corrector { path: p, stamp: None, pairs: Vec::new() };
        assert_eq!(c.apply("sempre con HCB, ma"), "sempre con DHCP, ma");
        assert_eq!(c.apply("HCBX e xhcb restano"), "HCBX e xhcb restano");
        assert_eq!(c.apply("niente di Ruter perché"), "niente di router perché");
        assert_eq!(c.apply("l'HCB è attivo"), "l'DHCP è attivo");
    }
}
