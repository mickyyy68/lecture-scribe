//! Slide themes. `theme/base.css` lays the slides out using design tokens only; `theme/default.css`
//! sets every token. A custom theme is a CSS file in `<data dir>/themes/` that overrides some of
//! the tokens. It is loaded after the default, in the app and in exported decks.
//!
//! Themes can be written by hand or made by Claude Code from a description (`create`).

use serde_json::{json, Value};
use std::path::PathBuf;

pub const BASE_CSS: &str = include_str!("theme/base.css");
pub const DEFAULT_CSS: &str = include_str!("theme/default.css");
const PROMPT: &str = include_str!("theme_prompt.txt");

pub fn dir() -> PathBuf {
    crate::paths::data_dir().join("themes")
}

/// Custom theme names (file stems), sorted.
pub fn list() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "css"))
        .filter_map(|p| Some(p.file_stem()?.to_string_lossy().to_string()))
        .collect();
    v.sort_by_key(|s| s.to_lowercase());
    v
}

/// The theme's CSS, or "" for the default theme or a missing file.
pub fn css(name: &str) -> String {
    match clean_name(name) {
        Ok(n) => std::fs::read_to_string(dir().join(format!("{n}.css"))).unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// The theme chosen in the settings.
pub fn current() -> String {
    css(crate::paths::load_settings()["theme"].as_str().unwrap_or(""))
}

pub fn delete(name: &str) -> Result<(), String> {
    let n = clean_name(name)?;
    std::fs::remove_file(dir().join(format!("{n}.css"))).map_err(|e| e.to_string())
}

/// A name that is safe as a file name: letters, digits, spaces and a few marks, at most 40.
fn clean_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    let ok = !n.is_empty()
        && n.chars().count() <= 40
        && !n.starts_with('.')
        && n.chars().all(|c| c.is_alphanumeric() || " -_&'.()+".contains(c));
    if ok { Ok(n.to_string()) } else { Err("Theme names can use letters, digits, spaces and - _ & ' . ( ) +, up to 40 characters.".into()) }
}

/// Asks Claude Code (the user's subscription) to write a theme from a description, such as
/// "navy and gold like my university, serif titles" or a website to take the colours from.
/// Saves it as `<name>.css` and returns `{ name, css, themes }`.
pub fn create(name: &str, description: &str, model: &str) -> Result<Value, String> {
    let name = clean_name(name)?;
    if description.trim().is_empty() {
        return Err("Describe the theme first.".into());
    }
    let work = std::env::temp_dir().join(format!("transcribe-lecture-theme-{}-{}", std::process::id(), chrono::Local::now().timestamp_millis()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let sys = work.join("system.txt");
    std::fs::write(&sys, PROMPT.replace("{default}", DEFAULT_CSS).replace("{base}", BASE_CSS)).map_err(|e| e.to_string())?;
    let mut cmd = crate::slides::claude_command(model, "medium", &work)?;
    // WebFetch only, so "colours from example.edu" works; nothing can touch the disk.
    cmd.arg("--system-prompt-file").arg(&sys).args(["--output-format", "json", "--tools", "WebFetch", "--allowedTools", "WebFetch"]);
    let out = crate::slides::run_piped(cmd, &format!("Theme name: {name}\n\n{}", description.trim()), 300);
    let _ = std::fs::remove_dir_all(&work);
    let (stdout, stderr) = out?;
    let r: Value = serde_json::from_str(stdout.trim()).map_err(|_| crate::slides::claude_failed(&stderr))?;
    if r["is_error"].as_bool() == Some(true) || r["subtype"] != "success" {
        return Err(format!("Claude Code: {}", r["result"].as_str().unwrap_or("error").chars().take(300).collect::<String>()));
    }
    let css = validate(r["result"].as_str().unwrap_or(""))?;
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    std::fs::write(dir().join(format!("{name}.css")), &css).map_err(|e| e.to_string())?;
    Ok(json!({ "name": name, "css": css, "themes": list() }))
}

/// The CSS out of the model's answer, if it looks like a theme: no Markdown fences, no markup.
fn validate(answer: &str) -> Result<String, String> {
    let mut s = answer.trim();
    if let Some(a) = s.find("```") {
        let rest = &s[a + 3..];
        let rest = rest.split_once('\n').map_or(rest, |(_, r)| r);
        s = rest.rfind("```").map_or(rest, |b| &rest[..b]).trim();
    }
    if !s.contains(":root") || !s.contains("--") {
        return Err("Claude Code didn't return a theme. Try describing it differently.".into());
    }
    if s.contains('<') || s.len() > 40_000 {
        return Err("Claude Code's theme isn't plain CSS, so it wasn't saved.".into());
    }
    Ok(format!("{s}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_answers() {
        assert!(clean_name("My university (2)").is_ok());
        assert!(clean_name("../x").is_err());
        assert!(clean_name("").is_err());
        let css = validate("Here it is:\n```css\n:root { --primary: red; }\n```\nEnjoy").unwrap();
        assert_eq!(css, ":root { --primary: red; }\n");
        assert!(validate(":root { --x: 1 } </style><script>").is_err());
        assert!(validate("Sorry, I can't.").is_err());
    }
}
