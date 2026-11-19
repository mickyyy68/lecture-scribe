//! Clean up: Claude rewrites the transcript into what the professor most likely said, as a new
//! version beside the original (see versions.rs).
//!
//! The lecture goes in parts of `PART` lines, a few at a time, each with some lines around it
//! for context. Only the lines' own text is sent, not the whole lecture: Claude Code's prompt
//! cache covers just the system prompt, so the whole transcript with every part would cost
//! several times as much. Each line comes back cleaned, or "" when it was only noise; a line
//! missing from the answer, or one that grew far longer than what was said, stays as it was.
//!
//! When the shown version doesn't cover the whole lecture (lines recorded since), Clean up does
//! just those lines and extends it; otherwise it makes a new version from the original.
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const PROMPT: &str = include_str!("clean_prompt.txt");
/// Lines per Claude Code run, lines of context on each side, and runs at a time.
const PART: usize = 100;
const CONTEXT: usize = 25;
const PARALLEL: usize = 4;

type Lines = [(String, String)];

/// The parts to clean: `from..to` in runs of about `PART` lines (a short last one joins the one
/// before it).
pub fn parts(from: usize, to: usize) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = (from..to).step_by(PART).map(|a| (a, (a + PART).min(to))).collect();
    if out.len() > 1 && out.last().is_some_and(|(a, b)| b - a < PART / 3) {
        let (_, b) = out.pop().unwrap();
        out.last_mut().unwrap().1 = b;
    }
    out
}

fn numbered(raw: &Lines, r: std::ops::Range<usize>) -> String {
    r.map(|i| format!("#{i} [{}] {}\n", raw[i].0, raw[i].1)).collect()
}

/// The request for lines `a..b`, with context around them.
pub fn request(raw: &Lines, a: usize, b: usize, glossary: &Value) -> String {
    let mut s = String::new();
    if a > 0 {
        s.push_str(&format!("CONTEXT BEFORE (don't return these lines):\n{}\n", numbered(raw, a.saturating_sub(CONTEXT)..a)));
    }
    s.push_str(&format!("LINES TO CLEAN:\n{}\n", numbered(raw, a..b)));
    if b < raw.len() {
        s.push_str(&format!("CONTEXT AFTER (don't return these lines):\n{}\n", numbered(raw, b..(b + CONTEXT).min(raw.len()))));
    }
    s.push_str("=== REQUEST ===\n");
    if glossary["corrections"].as_array().is_some_and(|a| !a.is_empty()) || glossary["terms"].as_array().is_some_and(|a| !a.is_empty()) {
        s.push_str(&format!("COURSE GLOSSARY (already known, don't repeat it):\n{}\n\n", json!({ "corrections": glossary["corrections"], "terms": glossary["terms"] })));
    }
    s.push_str(&format!("CLEAN lines #{a} to #{}: return every one of them, in order.", b - 1));
    s
}

/// The model's lines `a..b` as `(time, Some(clean text) | None for deleted)`. A line it didn't
/// return, or made much longer than the original (added content, not a clean up), stays as is.
pub fn lines_of(out: &Value, raw: &Lines, a: usize, b: usize) -> Vec<(String, Option<String>)> {
    let mut got: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    for l in out["lines"].as_array().into_iter().flatten() {
        let n = l["n"].as_u64().map(|n| n as usize).or_else(|| l["n"].as_str()?.trim_start_matches('#').parse().ok());
        if let (Some(n), Some(text)) = (n, l["text"].as_str()) {
            got.entry(n).or_insert_with(|| text.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }
    (a..b)
        .map(|i| {
            let (ts, old) = &raw[i];
            let text = match got.get(&i) {
                Some(t) if t.is_empty() => None,
                Some(t) if t.chars().count() <= old.chars().count() * 2 + 60 => Some(t.clone()),
                _ => Some(old.clone()),
            };
            (ts.clone(), text)
        })
        .collect()
}

fn add_usage(sum: &mut Value, u: &Value) {
    for k in ["prompt_tokens", "prompt_cache_hit_tokens", "prompt_cache_miss_tokens", "completion_tokens"] {
        sum[k] = json!(sum[k].as_u64().unwrap_or(0) + u[k].as_u64().unwrap_or(0));
    }
    sum["list_cost_usd"] = json!(sum["list_cost_usd"].as_f64().unwrap_or(0.0) + u["list_cost_usd"].as_f64().unwrap_or(0.0));
    for k in ["model", "effort"] {
        sum[k] = u[k].clone();
    }
}

/// Cleans up the lecture `t` and saves the result as a version, which becomes the one shown.
/// `progress(done, total)` is called as parts finish. Runs as the job "clean" (see
/// `slides::job`), so Stop kills its Claude Code runs. Returns the lecture as now shown
/// (`transcript_payload`) with `type: "cleaned"`, `suggestions` (glossary corrections for the
/// student to review), `stats` and `usage`.
pub fn clean(t: &Path, course: &str, model: &str, effort: &str, lang: &str, progress: &(dyn Fn(usize, usize) + Sync)) -> Result<Value, String> {
    let raw = crate::slides::parse_transcript(&std::fs::read_to_string(t).map_err(|e| format!("Can't read the transcript: {e}"))?).1;
    if raw.is_empty() {
        return Err("The transcript is empty.".into());
    }
    let info = crate::versions::info(t);
    let active = info["active"].as_str().and_then(|id| info["versions"].as_array()?.iter().find(|v| v["id"] == id)).cloned();
    let (from, extend) = match &active {
        Some(v) if (v["covers"].as_u64().unwrap_or(0) as usize) < raw.len() => (v["covers"].as_u64().unwrap() as usize, v["id"].as_str().map(str::to_string)),
        _ => (0, None),
    };
    let to = raw.len();
    let todo = parts(from, to);

    let g = crate::glossary::load(course);
    let docs = crate::material::load(course);
    let course_name = if course.trim().is_empty() { String::new() } else { format!(" of the course \"{}\"", course.trim()) };
    let mut system = PROMPT.replace("{course}", &course_name);
    if !docs.is_empty() {
        system.push_str(
            "\nCOURSE MATERIAL\nYou have the index of the course PDFs (below) and two tools: \"search_material\" (searches words, returns pages with an excerpt) and \"read_page\". \
             Use them sparingly, only to check how the course spells a term the transcript garbled. Never add content from them.\n\
             MATERIAL INDEX (file, page, first line):\n",
        );
        system.push_str(&crate::material::index(&docs));
    }
    // Rewriting garbled speech needs care: at least High, like the other whole-lecture jobs.
    let (model, mut effort) = crate::slides::model_effort(model, effort);
    if matches!(effort, "low" | "medium") {
        effort = "high";
    }

    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failed = std::sync::atomic::AtomicBool::new(false);
    let results: Mutex<Vec<Option<Result<(Vec<(String, Option<String>)>, Value, Value), String>>>> = Mutex::new(vec![None; todo.len()]);
    progress(0, todo.len());
    std::thread::scope(|s| {
        for _ in 0..PARALLEL.min(todo.len()) {
            s.spawn(|| {
                crate::slides::in_job("clean", || {
                    loop {
                        let k = next.fetch_add(1, Ordering::SeqCst);
                        if k >= todo.len() || crate::slides::cancelled("clean") || failed.load(Ordering::SeqCst) {
                            return;
                        }
                        let (a, b) = todo[k];
                        let user = request(&raw, a, b, &g);
                        // One more try if the answer wasn't usable (not after a Stop).
                        let mut r = Err(String::new());
                        for _ in 0..2 {
                            r = crate::slides::run_claude(&system, &user, course, !docs.is_empty(), model, effort, true)
                                .and_then(|run| Ok((crate::slides::answer_json(&run.content)?, run.usage)))
                                .map(|(out, usage)| (lines_of(&out, &raw, a, b), out["glossary"].clone(), usage));
                            if r.is_ok() || crate::slides::cancelled("clean") {
                                break;
                            }
                        }
                        let bad = r.is_err();
                        results.lock().unwrap()[k] = Some(r);
                        if bad {
                            failed.store(true, Ordering::SeqCst); // the others stop too
                            return;
                        }
                        progress(done.fetch_add(1, Ordering::SeqCst) + 1, todo.len());
                    }
                })
            });
        }
    });

    let (mut cleaned, mut suggested, mut terms, mut usage) = (Vec::new(), Vec::new(), Vec::new(), json!({}));
    let results = results.into_inner().unwrap();
    // The first real error, if a part failed; parts never started then count as stopped.
    if let Some(Err(e)) = results.iter().flatten().find(|r| r.is_err()) {
        return Err(e.clone());
    }
    for r in results {
        let (lines, glossary, u) = r.unwrap_or_else(|| Err(crate::slides::STOPPED.into()))?;
        cleaned.extend(lines);
        suggested.extend(glossary["corrections"].as_array().cloned().unwrap_or_default());
        terms.extend(glossary["terms"].as_array().cloned().unwrap_or_default());
        add_usage(&mut usage, &u);
    }
    let suggestions = crate::glossary::safe_corrections(course, &json!(suggested), lang);
    // Terms are only context for Claude's next requests: added without asking.
    crate::glossary::merge(course, &json!({ "terms": terms }));
    crate::versions::save_clean(t, from, to, &cleaned, extend.as_deref(), model)?;
    let changed = cleaned.iter().zip(&raw[from..]).filter(|((_, c), (_, o))| c.as_ref().is_some_and(|c| c != o)).count();
    let removed = cleaned.iter().filter(|(_, c)| c.is_none()).count();
    let mut v = crate::slides::transcript_payload(t)?;
    v["type"] = json!("cleaned");
    v["suggestions"] = json!(suggestions);
    v["stats"] = json!({ "lines": to - from, "changed": changed, "removed": removed, "extended": extend.is_some() });
    v["usage"] = crate::slides::usage_summary(&usage);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_cover_the_lines_without_a_tiny_last_one() {
        assert_eq!(parts(0, 250), [(0, 100), (100, 200), (200, 250)]);
        assert_eq!(parts(0, 210), [(0, 100), (100, 210)]);
        assert_eq!(parts(40, 90), [(40, 90)]);
        assert!(parts(5, 5).is_empty());
    }

    #[test]
    fn the_request_numbers_lines_and_marks_the_context() {
        let raw: Vec<(String, String)> = (0..300).map(|i| (format!("00:00:{:02}", i % 60), format!("line {i}"))).collect();
        let r = request(&raw, 100, 200, &json!({ "corrections": [], "terms": [] }));
        assert!(r.contains("CONTEXT BEFORE") && r.contains("#75 [") && !r.contains("#74 ["));
        assert!(r.contains("LINES TO CLEAN:\n#100 [00:00:40] line 100\n"));
        assert!(r.contains("#224 [") && !r.contains("#225 ["));
        assert!(r.ends_with("CLEAN lines #100 to #199: return every one of them, in order."));
        assert!(!r.contains("COURSE GLOSSARY"));
        assert!(!request(&raw, 0, 100, &json!({})).contains("CONTEXT BEFORE"));
    }

    /// Deleted lines, missing ones (kept), "#n" numbers, and a line that grew into new content.
    #[test]
    fn answers_become_lines_safely() {
        let raw: Vec<(String, String)> = ["the radiant", "can you know", "eater", "short"].iter().enumerate().map(|(i, x)| (format!("00:00:0{i}"), x.to_string())).collect();
        let out = json!({ "lines": [
            { "n": 0, "text": " the  gradient " },
            { "n": "#1", "text": "" },
            { "n": 3, "text": "short, and here is a whole paragraph the professor never said, added by the model to explain the idea at length" },
            { "n": 0, "text": "a second answer for #0 is ignored" },
            { "n": 9, "text": "not asked for" },
        ]});
        let got = lines_of(&out, &raw, 0, 4);
        let texts: Vec<Option<&str>> = got.iter().map(|(_, x)| x.as_deref()).collect();
        assert_eq!(texts, [Some("the gradient"), None, Some("eater"), Some("short")]);
        assert_eq!(got[1].0, "00:00:01");
    }
}
