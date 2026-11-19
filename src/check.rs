//! "Check transcript": Claude Code proofreads a lecture's transcript for recognition errors.
//!
//! It proposes corrected lines and new glossary corrections; nothing is written here. The
//! student reviews them in the Glossary tab: accepted lines are saved like hand edits
//! (`lineEdit`, so it works during a recording) and accepted corrections go to the glossary.
use serde_json::{json, Value};
use std::path::Path;

const PROMPT: &str = include_str!("check_prompt.txt");

/// Proofreads the lecture. Returns `{type: "checkResult", path, fixes: [{i, t, old, text, why}],
/// corrections: [{heard, correct}], terms: [..], usage, lookups}`.
pub fn check(t: &Path, course: &str, model: &str, effort: &str) -> Result<Value, String> {
    let (_, lines) = crate::slides::read_transcript(t)?;
    if lines.is_empty() {
        return Err("The transcript is empty.".into());
    }
    let numbered: String = lines.iter().enumerate().map(|(i, (ts, x))| format!("#{i} [{ts}] {x}\n")).collect();
    let g = crate::glossary::load(course);
    let mut user = format!("LECTURE TRANSCRIPT:\n{numbered}\n=== REQUEST ===\n");
    if g["corrections"].as_array().is_some_and(|a| !a.is_empty()) || g["terms"].as_array().is_some_and(|a| !a.is_empty()) {
        user.push_str(&format!("COURSE GLOSSARY (already known, don't repeat it):\n{}\n\n", json!({ "corrections": g["corrections"], "terms": g["terms"] })));
    }
    user.push_str(&format!("CHECK: proofread lines #0 to #{} for recognition errors.", lines.len() - 1));

    let docs = crate::material::load(course);
    let course_name = if course.trim().is_empty() { String::new() } else { format!(" of the course \"{}\"", course.trim()) };
    let mut system = PROMPT.replace("{course}", &course_name);
    if !docs.is_empty() {
        system.push_str(
            "\nCOURSE MATERIAL\nYou have the index of the course PDFs (below) and two tools: \"search_material\" (searches words, returns pages with an excerpt) and \"read_page\". \
             Use them sparingly, only to check how the course spells a term the transcript garbled (search the heard form or the topic). Never add content from them.\n\
             MATERIAL INDEX (file, page, first line):\n",
        );
        system.push_str(&crate::material::index(&docs));
    }
    // Proofreading needs care: at low effort it finds a tenth of the errors (tested on a real lecture).
    let (model, mut effort) = crate::slides::model_effort(model, effort);
    if matches!(effort, "low" | "medium") {
        effort = "high";
    }
    let run = crate::slides::run_claude(&system, &user, course, !docs.is_empty(), model, effort, true)?;
    let out = crate::slides::answer_json(&run.content)?;
    let known: Vec<String> = g["corrections"].as_array().into_iter().flatten().filter_map(|c| c["heard"].as_str()).map(str::to_lowercase).collect();
    Ok(json!({
        "type": "checkResult",
        "path": t.to_string_lossy(),
        "fixes": fixes(&out, &lines),
        "corrections": out["glossary"]["corrections"].as_array().into_iter().flatten()
            .filter_map(|c| Some((c["heard"].as_str()?.trim(), c["correct"].as_str()?.trim())))
            .filter(|(h, c)| crate::glossary::valid_correction(h, c) && !known.contains(&h.to_lowercase()))
            .map(|(h, c)| json!({ "heard": h, "correct": c }))
            .collect::<Vec<_>>(),
        "terms": out["glossary"]["terms"],
        "usage": crate::slides::usage_summary(&run.usage),
        "lookups": run.lookups,
    }))
}

/// The model's fixes that are safe to offer: an existing line, really changed, and still the
/// same line (a few words fixed, not rewritten). Each keeps the line's old text, so the app
/// can tell when the line was edited since.
pub fn fixes(out: &Value, lines: &[(String, String)]) -> Vec<Value> {
    let secs = |t: &str| t.split(':').fold(0u64, |a, x| a * 60 + x.parse::<u64>().unwrap_or(0));
    let mut seen = std::collections::HashSet::new();
    out["fixes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let i = f["line"].as_u64().or_else(|| f["line"].as_str()?.trim_start_matches('#').parse().ok())? as usize;
            let (ts, old) = lines.get(i)?;
            let text = f["text"].as_str()?.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() || text == *old {
                return None;
            }
            // A proofread, not a rewrite: at most about a third of the line changed.
            let n = old.chars().count().max(text.chars().count());
            if crate::material::lev(old, &text) > (n / 3).max(12) || !seen.insert(i) {
                return None;
            }
            Some(json!({ "i": i, "t": secs(ts), "old": old, "text": text, "why": f["why"].as_str().unwrap_or("").trim() }))
        })
        .take(150)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_safe_fixes() {
        let lines: Vec<(String, String)> = [
            ("00:00:01", "configure the HCB server first"),
            ("00:00:09", "then we talk about routing"),
            ("00:00:15", "the sin packet opens the connection"),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let out = json!({ "fixes": [
            { "line": 0, "text": "configure the DHCP server first", "why": "HCB → DHCP" },
            { "line": 1, "text": "then we talk about routing" },                                  // unchanged
            { "line": 2, "text": "A completely different sentence that the professor never said at all." }, // rewrite
            { "line": "#2", "text": "the SYN  packet opens the connection", "why": "sin → SYN" },
            { "line": 0, "text": "configure the DHCP server" },                                   // same line twice
            { "line": 9, "text": "out of range" },
        ]});
        let f = fixes(&out, &lines);
        let got: Vec<(u64, &str)> = f.iter().map(|x| (x["i"].as_u64().unwrap(), x["text"].as_str().unwrap())).collect();
        assert_eq!(got, [(0, "configure the DHCP server first"), (2, "the SYN packet opens the connection")]);
        assert_eq!(f[1]["t"], 15);
        assert_eq!(f[1]["old"], "the sin packet opens the connection");
    }
}
