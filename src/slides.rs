//! Slides: Claude Code turns the transcript into a deck, on demand.
//!
//! The deck lives next to the transcript as "<lecture> · Slide.json" (one batch per click, so
//! the last click can be undone) and is exported to "<lecture> · Slide.html" after every change.

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const SLIDES_CSS: &str = include_str!("slides.css");
pub const SLIDES_JS: &str = include_str!("slides.js");
pub const KATEX_CSS: &str = include_str!("katex/katex.css");
pub const KATEX_JS: &str = include_str!("katex/katex.min.js");
const DECK_HTML: &str = include_str!("deck.html");
pub(crate) const PROMPT: &str = include_str!("slides_prompt.txt");
const PROMPT_MATERIAL: &str = include_str!("slides_prompt_material.txt");
const SAMPLE_LECTURE: &str = include_str!("sample_lecture.md");
/// Claude Code model aliases and reasoning efforts offered in the app.
pub const MODELS: &[&str] = &["sonnet", "opus", "fable", "haiku"];
pub const EFFORTS: &[&str] = &["low", "medium", "high", "max"];
pub const DEFAULT_MODEL: &str = "sonnet";
pub const DEFAULT_EFFORT: &str = "low";
const SUFFIX: &str = " · Slide";

pub fn deck_path(transcript: &Path, ext: &str) -> PathBuf {
    let stem = transcript.file_stem().unwrap_or_default().to_string_lossy();
    transcript.with_file_name(format!("{stem}{SUFFIX}{ext}"))
}

/// Title and finished lines `(hh:mm:ss, text)`. The live partial line is the text after the
/// last blank line, so it is never included.
pub fn parse_transcript(text: &str) -> (String, Vec<(String, String)>) {
    let title = text.lines().next().and_then(|l| l.strip_prefix("# ")).unwrap_or("Lecture").trim().to_string();
    let mut parts: Vec<&str> = text.split("\n\n").collect();
    parts.pop();
    let lines = parts
        .into_iter()
        .filter_map(|p| {
            let p = p.trim();
            let rest = p.strip_prefix("**[")?;
            let (t, txt) = rest.split_once("]** ")?;
            Some((t.to_string(), txt.trim().to_string()))
        })
        .collect();
    (title, lines)
}

pub(crate) fn read_transcript(p: &Path) -> Result<(String, Vec<(String, String)>), String> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("Can't read the transcript: {e}"))?;
    Ok(parse_transcript(&text))
}

fn load_deck(t: &Path) -> Value {
    std::fs::read_to_string(deck_path(t, ".json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .filter(|v: &Value| v["batches"].is_array())
        .unwrap_or_else(|| json!({ "batches": [] }))
}

/// The deck as shown: the lecture's slides, then the latest review cards (always at the end).
fn slides_of(deck: &Value) -> Vec<Value> {
    let (mut out, review) = parts_of(deck);
    out.extend(review);
    out
}

/// (lecture slides, review cards). A `review: true` batch replaces earlier review cards and
/// leaves the lecture slides alone.
fn parts_of(deck: &Value) -> (Vec<Value>, Vec<Value>) {
    let (mut out, mut review): (Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new());
    for b in deck["batches"].as_array().into_iter().flatten() {
        let slides = b["slides"].as_array().cloned().unwrap_or_default();
        if b["review"].as_bool() == Some(true) {
            review = slides;
            continue;
        }
        if b["rebuild"].as_bool() == Some(true) {
            out.clear();
        }
        if b["replace_last"].as_bool() == Some(true) {
            out.pop();
        }
        out.extend(slides);
    }
    (out, review)
}

fn done_of(deck: &Value) -> usize {
    deck["batches"].as_array().and_then(|b| b.last()).and_then(|b| b["to_line"].as_u64()).unwrap_or(0) as usize
}

fn write_atomic(p: &Path, s: &str) -> Result<(), String> {
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, s).and_then(|_| std::fs::rename(&tmp, p)).map_err(|e| format!("Couldn't save: {e}"))
}

pub fn foot(course: &str, title: &str) -> String {
    [course.trim(), title.trim()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" · ")
}

/// The transcript's title, or the model's suggestion when the lecture wasn't given one.
fn display_title(deck: &Value, title: &str) -> String {
    let generic = matches!(title.trim().to_lowercase().as_str(), "" | "lecture" | "lezione" | "simulation");
    match deck["lecture_title"].as_str() {
        Some(s) if generic && !s.trim().is_empty() => s.trim().to_string(),
        _ => title.to_string(),
    }
}

fn save(t: &Path, deck: &Value, title: &str) -> Result<(), String> {
    write_atomic(&deck_path(t, ".json"), &serde_json::to_string_pretty(deck).unwrap())?;
    let title = display_title(deck, title);
    let slides = slides_of(deck);
    let course = deck["course"].as_str().unwrap_or("");
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let js_json = |v: &Value| v.to_string().replace("</", "<\\/");
    let html = DECK_HTML
        .replace("{{LANG}}", &esc(deck["lang"].as_str().unwrap_or("en")))
        .replace("{{TITLE}}", &esc(&format!("{title} · Slides")))
        .replace("{{BUNDLE}}", crate::theme::BASE_CSS)
        .replace("{{TOKENS}}", crate::theme::DEFAULT_CSS)
        .replace("{{THEME}}", &crate::theme::current().replace("</", "<\\/"))
        .replace("{{SLIDES_CSS}}", SLIDES_CSS)
        .replace("{{SLIDES_JS}}", SLIDES_JS)
        .replace("{{FOOT}}", &js_json(&json!(foot(course, &title))))
        .replace("{{DATA}}", &js_json(&Value::Array(slides)));
    // KaTeX (≈640 KB) only goes into decks that have math; it is inserted last, after the data.
    let data = Value::Array(slides_of(deck)).to_string();
    let math = data.contains("\"type\":\"math\"") || data.contains('$');
    let html = html.replace("{{KATEX_CSS}}", if math { KATEX_CSS } else { "" }).replace("{{KATEX_JS}}", if math { KATEX_JS } else { "" });
    write_atomic(&deck_path(t, ".html"), &html)
}

/// Token counts. Runs are covered by the Claude subscription; "listCost" is what the same tokens
/// would cost on the API.
pub(crate) fn usage_summary(u: &Value) -> Value {
    let n = |k: &str| u[k].as_u64().unwrap_or(0) as f64;
    let (hit, miss, out) = (n("prompt_cache_hit_tokens"), n("prompt_cache_miss_tokens"), n("completion_tokens"));
    json!({ "model": u["model"], "effort": u["effort"], "prompt": hit + miss, "cached": hit, "output": out,
        "listCost": u["list_cost_usd"].as_f64().unwrap_or(0.0), "cacheRate": if hit + miss > 0.0 { hit / (hit + miss) } else { 0.0 } })
}

/// What the slide panel shows for this transcript.
pub fn state(t: &Path) -> Result<Value, String> {
    let (title, lines) = read_transcript(t)?;
    let deck = load_deck(t);
    let last = deck["batches"].as_array().and_then(|b| b.last());
    Ok(json!({
        "type": "slides",
        "path": t.to_string_lossy(),
        "title": display_title(&deck, &title),
        "lines": lines.len(),
        "done": done_of(&deck),
        "lastT": lines.last().map(|l| l.0.clone()),
        "slides": slides_of(&deck),
        "lang": deck["lang"],
        "html": deck_path(t, ".html").to_string_lossy(),
        "usage": last.map(|b| usage_summary(&b["usage"])),
        "flagged": last.and_then(|b| b["flagged"].as_u64()).unwrap_or(0),
        "lookups": last.map(|b| b["lookups"].clone()).unwrap_or(json!([])),
    }))
}

/// Writes the deck's HTML again, e.g. after the theme changed.
pub fn export(t: &Path) -> Result<(), String> {
    let deck = load_deck(t);
    if deck["batches"].as_array().is_none_or(|b| b.is_empty()) {
        return Ok(());
    }
    let (title, _) = read_transcript(t)?;
    save(t, &deck, &title)
}

pub fn undo(t: &Path) -> Result<Value, String> {
    let mut deck = load_deck(t);
    if let Some(b) = deck["batches"].as_array_mut() {
        b.pop();
    }
    let (title, _) = read_transcript(t)?;
    save(t, &deck, &title)?;
    state(t)
}

/// Sends the transcript to Claude Code (`run_claude`) and adds the new slides. With `rebuild`, the
/// whole deck is made again from the full lecture; undo brings the old one back.
///
/// `model` is a Claude Code model alias ("sonnet", "opus", …) and `effort` its reasoning effort.
/// Rebuild and Review cover the whole lecture, so they run at least at "high".
///
/// The request is laid out from most stable to least stable, so prompt caching works: system
/// prompt (fixed per course and language) → the full transcript so far (append-only) →
/// glossary, deck state and the instruction (change every call).
///
/// `lang` is the slides' language as an ISO code ("it", "en"); "" means the lecture's own.
///
/// `Mode::Review` makes end-of-lecture review cards from the whole lecture. They are their own
/// batch (`review: true`), shown after the slides, and don't move the "new lines" mark.
///
/// `hint` is the student's free-text suggestion for this click ("more diagrams", "skip the
/// recap"); it goes in the per-call tail and never overrides fidelity.
#[allow(clippy::too_many_arguments)]
pub fn generate(t: &Path, course: &str, mode: Mode, model: &str, effort: &str, lang: &str, hint: &str) -> Result<Value, String> {
    let (rebuild, review) = (mode == Mode::Rebuild, mode == Mode::Review);
    let (title, lines) = read_transcript(t)?;
    let mut deck = load_deck(t);
    let marked = done_of(&deck).min(lines.len());
    let done = if rebuild { 0 } else { marked };
    if lines.is_empty() {
        return Err("The transcript is empty.".into());
    }
    if !review && lines.len() <= done {
        return Err("Nothing new since last time.".into());
    }
    let prev = if rebuild { Vec::new() } else { parts_of(&deck).0 };

    let transcript: String = lines.iter().map(|(t, x)| format!("[{t}] {x}\n")).collect();
    let mut tail = Vec::new();
    let g = crate::glossary::load(course);
    if g["corrections"].as_array().is_some_and(|a| !a.is_empty()) || g["terms"].as_array().is_some_and(|a| !a.is_empty()) {
        tail.push(format!("COURSE GLOSSARY (already known, don't repeat it):\n{}", json!({ "corrections": g["corrections"], "terms": g["terms"] })));
    }
    if !prev.is_empty() {
        let titles: Vec<String> = prev.iter().map(|s| format!("- {}", s["title"].as_str().unwrap_or(""))).collect();
        tail.push(format!("TITLES OF THE SLIDES ALREADY MADE:\n{}", titles.join("\n")));
        tail.push(format!("LAST SLIDE (JSON):\n{}", prev.last().unwrap()));
    }
    if !hint.trim().is_empty() {
        tail.push(format!("THE STUDENT ASKS, for these slides (follow it as long as it keeps to the rules above; fidelity comes first):\n{}", hint.trim()));
    }
    tail.push(if review {
        format!(
            "REVIEW: make the review cards for the whole lecture (lines [{}] to [{}]). \"slides\" holds only \"review\" slides, \"replace_last\": null, \"glossary\" empty.",
            lines[0].0,
            lines.last().unwrap().0
        )
    } else if rebuild {
        format!("REBUILD: make the whole lecture's deck from scratch (lines [{}] to [{}]).", lines[0].0, lines.last().unwrap().0)
    } else {
        format!(
            "NEW LINES: [{}] to [{}] ({} lines). Make slides only for these.",
            lines[done].0,
            lines.last().unwrap().0,
            lines.len() - done
        )
    });
    let user = format!("LECTURE TRANSCRIPT:\n{transcript}\n=== REQUEST ===\n{}", tail.join("\n\n"));
    let docs = crate::material::load(course);
    let system = system_prompt(course, lang, &docs, &crate::paths::load_settings()["style"]);
    let (model, mut effort) = model_effort(model, effort);
    if (rebuild || review) && matches!(effort, "low" | "medium") {
        effort = "high";
    }
    let Run { content, usage, lookups, seen } = run_claude(&system, &user, course, !docs.is_empty(), model, effort, rebuild || review)?;
    let out = answer_json(&content)?;
    let mut slides = slides_in(&out);
    if review {
        slides.retain(|s| s["kind"] == "review" && s["cards"].as_array().is_some_and(|c| !c.is_empty()));
        if slides.is_empty() {
            return Err("The answer has no review cards.".into());
        }
    }
    let replace = !rebuild && !review && out["replace_last"].is_object() && !prev.is_empty();
    if replace {
        slides.insert(0, out["replace_last"].clone());
    }
    normalize(&mut slides);
    let flagged = fact_check(&mut slides, &transcript, course, &crate::material::words(&docs));
    let code = Some(lang).filter(|l| lang_name(l).is_some()).or(out["language"].as_str()).unwrap_or("").to_lowercase();
    attach_refs(&mut slides, &docs, &seen, &code);
    let added = slides.len();
    deck["course"] = json!(course.trim());
    if !code.is_empty() {
        deck["lang"] = json!(code);
    }
    if let Some(lt) = out["lecture_title"].as_str().filter(|s| !s.trim().is_empty()) {
        if rebuild || deck["lecture_title"].as_str().is_none() {
            deck["lecture_title"] = json!(lt.trim());
        }
    }
    deck["batches"].as_array_mut().unwrap().push(json!({
        "from_line": done,
        "to_line": if review { marked } else { lines.len() },
        "replace_last": replace,
        "rebuild": rebuild,
        "review": review,
        "slides": slides,
        "flagged": flagged,
        "lookups": lookups,
        "usage": usage,
    }));
    save(t, &deck, &title)?;
    let (nc, nt) = if review { (0, 0) } else { crate::glossary::merge(course, &out["glossary"]) };
    let mut st = state(t)?;
    st["learned"] = json!({ "corrections": nc, "terms": nt });
    st["added"] = json!(added);
    st["replaced"] = json!(replace);
    st["rebuilt"] = json!(rebuild);
    st["reviewed"] = json!(review);
    Ok(st)
}

pub(crate) fn model_effort<'a>(model: &'a str, effort: &'a str) -> (&'a str, &'a str) {
    (if MODELS.contains(&model) { model } else { DEFAULT_MODEL }, if EFFORTS.contains(&effort) { effort } else { DEFAULT_EFFORT })
}

/// The slide prompt for a course and slide language, with the course material's index and the
/// student's slide style (`settings.style`, see `style_rules`).
fn system_prompt(course: &str, lang: &str, docs: &[crate::material::Doc], style: &Value) -> String {
    let course_name = if course.trim().is_empty() { String::new() } else { format!(" of the course \"{}\"", course.trim()) };
    let language = lang_name(lang).unwrap_or("the language the lecture is given in");
    let mut system = PROMPT.replace("{course}", &course_name).replace("{language}", language);
    if !docs.is_empty() {
        system.push('\n');
        system.push_str(PROMPT_MATERIAL);
        system.push_str("\nMATERIAL INDEX (file, page, first line):\n");
        system.push_str(&crate::material::index(docs));
    }
    system.push_str(&style_rules(style));
    system
}

/// The student's slide style as prompt rules. Each control's default adds nothing, so with the
/// defaults and no own rules the prompt is the plain one.
///
/// `style` = {"density": "minimal"|"balanced"|"detailed", "titles": "statement"|"topic"|"question",
/// "items": "terse"|"sentences", "notes": "none"|"short"|"full", "visuals": "text"|"balanced"|"diagrams",
/// "rules": free text}.
pub fn style_rules(style: &Value) -> String {
    let get = |k: &str| style[k].as_str().unwrap_or("");
    let mut r: Vec<&str> = Vec::new();
    r.push(match get("density") {
        "minimal" => "Density: minimal. Lists of at most 3 items of at most ~35 characters, usually one block per column. Prefer more slides with less on each.",
        "detailed" => "Density: detailed. Lists of up to 5 items of up to ~80 characters, use the subtitle, and fit a second block in a column when there is room. Fewer, fuller slides (still within the body height).",
        _ => "",
    });
    r.push(match get("titles") {
        "topic" => "Titles: name the topic or mechanism in 1-5 words (\"Softmax\", \"Three-way handshake\"), not a full claim.",
        "question" => "Titles: a short question the slide answers (\"Why normalise the scores?\", \"What does TTL prevent?\"), as a study prompt.",
        _ => "",
    });
    r.push(match get("items") {
        "sentences" => "List items: complete short sentences with a verb, each understandable on its own when studying weeks later.",
        "terse" => "List items: telegraphic phrases (no articles where the language allows, no full stops); the meaning must stay clear.",
        _ => "",
    });
    r.push(match get("notes") {
        "none" => "Notes: leave \"notes\" empty, except for a detail the student would really miss.",
        "full" => "Notes: 3-5 sentences that retell the professor's explanation in order, with the examples given, as a text to study from.",
        _ => "",
    });
    r.push(match get("visuals") {
        "text" => "Visuals: prefer lists, definitions and formulas; draw a diagram only when the professor clearly describes one.",
        "diagrams" => "Visuals: whenever something can be drawn (a process, structure, comparison, function or exchange), draw it, so most slides carry a diagram, plot, table or formula. Still only with what was said (FIDELITY).",
        _ => "",
    });
    r.retain(|x| !x.is_empty());
    let own = get("rules").trim();
    if r.is_empty() && own.is_empty() {
        return String::new();
    }
    let mut out = String::from("\nSTYLE (the student's choices: they override FEW WORDS and VARIETY AND LAYOUT where they conflict, never FIDELITY, the block limits or the FORMAT)\n");
    for x in r {
        out.push_str(&format!("- {x}\n"));
    }
    if !own.is_empty() {
        out.push_str(&format!("The student's own rules:\n{own}\n"));
    }
    out
}

/// The JSON object in a model answer (models sometimes wrap it in prose or a code fence).
pub(crate) fn answer_json(content: &str) -> Result<Value, String> {
    match (content.find('{'), content.rfind('}')) {
        (Some(a), Some(b)) if b > a => serde_json::from_str(&content[a..=b]).map_err(|e| format!("The slide JSON isn't valid: {e}")),
        _ => Err("The answer has no JSON.".into()),
    }
}

fn slides_in(out: &Value) -> Vec<Value> {
    out["slides"].as_array().cloned().unwrap_or_default().into_iter().filter(|s| s.is_object()).collect()
}

/// Models sometimes give a column as a single block instead of a list.
fn normalize(slides: &mut [Value]) {
    for s in slides.iter_mut() {
        for col in ["left", "right"] {
            if s[col].is_object() {
                s[col] = json!([s[col].take()]);
            }
        }
    }
}

/// The excerpt the style samples are made from: about three minutes from a third of the way into
/// the latest lecture with enough text, or the built-in sample. Returns (label, lines).
fn sample_excerpt(dir: &Path, builtin: bool) -> (String, Vec<(String, String)>) {
    let secs = |t: &str| t.split(':').fold(0u64, |a, x| a * 60 + x.parse::<u64>().unwrap_or(0));
    if !builtin {
        for r in recent(dir) {
            let p = PathBuf::from(r["path"].as_str().unwrap_or(""));
            let Ok((_, lines)) = read_transcript(&p) else { continue };
            if lines.len() < 30 {
                continue;
            }
            let start = lines.len() / 3;
            let t0 = secs(&lines[start].0);
            let (mut chars, mut out) = (0, Vec::new());
            for l in &lines[start..] {
                if !out.is_empty() && (secs(&l.0) > t0 + 180 || chars > 3500) {
                    break;
                }
                chars += l.1.len();
                out.push(l.clone());
            }
            let label = format!("{}, {} – {}", r["name"].as_str().unwrap_or(""), out[0].0, out.last().unwrap().0);
            return (label, out);
        }
    }
    ("Built-in sample: Gradient descent".into(), parse_transcript(SAMPLE_LECTURE).1)
}

/// Three sample slides in the given style, for the Slide style page. With `feedback`, the
/// previous sample and the student's comment go along, and the answer also carries the own rules
/// rewritten to include it ("rules"), which the page puts back in its text box.
#[allow(clippy::too_many_arguments)]
pub fn sample(dir: &Path, builtin: bool, course: &str, model: &str, effort: &str, lang: &str, style: &Value, feedback: &str, previous: &Value) -> Result<Value, String> {
    let (label, lines) = sample_excerpt(dir, builtin);
    let transcript: String = lines.iter().map(|(t, x)| format!("[{t}] {x}\n")).collect();
    let mut req = vec![format!(
        "SAMPLE: make exactly 3 slides from these lines, as you would in the real deck, so the student can judge the style. \
         No cover slides. If the lines allow it, let one slide show a diagram, plot or formula. \"replace_last\": null, \"glossary\" empty."
    )];
    let feedback = feedback.trim();
    if !feedback.is_empty() {
        if previous.is_array() {
            req.push(format!("PREVIOUS SAMPLE (JSON):\n{previous}"));
        }
        req.push(format!(
            "STUDENT FEEDBACK on the previous sample: «{feedback}»\n\
             Apply it in the new sample. Also return \"rules\": the student's own rules (shown in STYLE, possibly none) rewritten to include this feedback: \
             short, general rules about how all their slides should be, one per line starting with \"- \", at most 8, in the language of the feedback. \
             Keep the earlier rules unless the feedback contradicts them; nothing specific to this excerpt; don't repeat the STYLE choices above."
        ));
    }
    let user = format!("LECTURE TRANSCRIPT (an excerpt):\n{transcript}\n=== REQUEST ===\n{}", req.join("\n\n"));
    let system = system_prompt(course, lang, &[], style);
    let (model, effort) = model_effort(model, effort);
    let run = run_claude(&system, &user, course, false, model, effort, false)?;
    let out = answer_json(&run.content)?;
    let mut slides = slides_in(&out);
    slides.retain(|s| s["kind"] != "review");
    slides.truncate(3);
    if slides.is_empty() {
        return Err("Claude Code returned no slides.".into());
    }
    normalize(&mut slides);
    fact_check(&mut slides, &transcript, course, &Default::default());
    let mut ev = json!({ "type": "styleSample", "slides": slides, "source": label, "usage": usage_summary(&run.usage),
        "lang": Some(lang).filter(|l| lang_name(l).is_some()).or(out["language"].as_str()).unwrap_or("") });
    if !feedback.is_empty() {
        ev["rules"] = json!(out["rules"].as_str().map(str::trim).unwrap_or(style["rules"].as_str().unwrap_or("")));
    }
    Ok(ev)
}

/// Languages the slides can be written in (Parakeet V3's), by ISO code.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"), ("it", "Italian"), ("fr", "French"), ("de", "German"), ("es", "Spanish"), ("pt", "Portuguese"),
    ("nl", "Dutch"), ("pl", "Polish"), ("ro", "Romanian"), ("ru", "Russian"), ("uk", "Ukrainian"), ("sv", "Swedish"),
    ("da", "Danish"), ("fi", "Finnish"), ("el", "Greek"), ("cs", "Czech"), ("sk", "Slovak"), ("sl", "Slovenian"),
    ("hr", "Croatian"), ("bg", "Bulgarian"), ("hu", "Hungarian"), ("et", "Estonian"), ("lv", "Latvian"), ("lt", "Lithuanian"),
    ("mt", "Maltese"),
];

pub(crate) fn lang_name(code: &str) -> Option<&'static str> {
    LANGUAGES.iter().find(|(c, _)| c.eq_ignore_ascii_case(code.trim())).map(|(_, n)| *n)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Slides for the lines since the last click.
    Next,
    /// The whole deck again, at higher effort.
    Rebuild,
    /// End-of-lecture review cards.
    Review,
}

impl Mode {
    pub fn parse(s: &str) -> Mode {
        match s {
            "rebuild" => Mode::Rebuild,
            "review" => Mode::Review,
            _ => Mode::Next,
        }
    }
}

/// What a model run returns: the answer text, token usage, the lookups made and the material
/// pages seen (only those may be cited).
pub(crate) struct Run {
    pub content: String,
    pub usage: Value,
    pub lookups: Vec<String>,
    pub seen: std::collections::HashSet<(String, usize)>,
}

/// The `claude` CLI. Apps started from Finder get a bare PATH, so look in the usual places first.
pub fn claude_bin() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let known = [home.join(".local/bin/claude"), home.join(".claude/local/claude"), "/opt/homebrew/bin/claude".into(), "/usr/local/bin/claude".into()];
    if let Some(p) = known.into_iter().find(|p| p.is_file()) {
        return Some(p);
    }
    let out = Command::new("/bin/zsh").args(["-lc", "command -v claude"]).output().ok()?;
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!p.is_empty()).then(|| PathBuf::from(p))
}

/// `claude -p` in `dir`, on the user's subscription and isolated from their own Claude Code
/// setup: no settings, MCP servers or CLAUDE.md of theirs, and no saved session. The caller adds
/// the tools and output format.
pub fn claude_command(model: &str, effort: &str, dir: &Path) -> Result<Command, String> {
    let bin = claude_bin().ok_or("Claude Code wasn't found. Install it and log in once with `claude` in Terminal.")?;
    let mut cmd = Command::new(bin);
    cmd.current_dir(dir)
        .args(["-p", "--model", model, "--effort", effort])
        .args(["--strict-mcp-config", "--no-session-persistence", "--setting-sources", "project"])
        // The subscription, never an API key that happens to be in the environment.
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT");
    Ok(cmd)
}

/// Kills `pid` after `secs` unless the returned flag is set first.
fn kill_after(pid: u32, secs: u64) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = done.clone();
    std::thread::spawn(move || {
        for _ in 0..secs {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
        }
        let _ = Command::new("kill").arg(pid.to_string()).status();
    });
    done
}

/// Runs `cmd` with `input` on stdin; returns (stdout, stderr). Killed after `secs`.
pub fn run_piped(mut cmd: Command, input: &str, secs: u64) -> Result<(String, String), String> {
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("Claude Code won't start: {e}"))?;
    let fed = child.stdin.take().unwrap().write_all(input.as_bytes());
    let done = kill_after(child.id(), secs);
    let out = child.wait_with_output().map_err(|e| e.to_string());
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    let out = out?;
    fed.map_err(|e| format!("Claude Code: {e}"))?;
    Ok((String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string()))
}

/// The error for a Claude Code run that ended without a result.
pub fn claude_failed(stderr: &str) -> String {
    let tail: String = stderr.trim().chars().rev().take(300).collect::<Vec<_>>().into_iter().rev().collect();
    format!("Claude Code stopped{}", if tail.is_empty() { ".".into() } else { format!(": {tail}") })
}

/// Runs the request through Claude Code (`claude -p`) on the user's Claude subscription, without
/// its own tools, settings or CLAUDE.md, in an empty temp directory. The course material is
/// offered through this app's MCP server (`--mcp-material`). Claude Code caches the prompt itself.
pub(crate) fn run_claude(system: &str, user: &str, course: &str, material: bool, model: &str, effort: &str, rebuild: bool) -> Result<Run, String> {
    use std::io::{BufRead, Read};
    let dir = std::env::temp_dir().join(format!("transcribe-lecture-claude-{}-{}", std::process::id(), chrono::Local::now().timestamp_millis()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let sys_file = dir.join("system.txt");
    std::fs::write(&sys_file, system).map_err(|e| e.to_string())?;
    let mut cmd = claude_command(model, effort, &dir)?;
    cmd.args(["--output-format", "stream-json", "--verbose", "--tools", ""]).arg("--system-prompt-file").arg(&sys_file);
    if material {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let cfg = json!({ "mcpServers": { "material": { "type": "stdio", "command": exe, "args": ["--mcp-material", "--course", course] } } });
        cmd.arg("--mcp-config").arg(cfg.to_string()).args(["--allowedTools", "mcp__material__search_material,mcp__material__read_page"]);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Claude Code won't start: {e}"))?;
    // The prompt goes in through stdin (it can be longer than a command line allows).
    let fed = child.stdin.take().unwrap().write_all(user.as_bytes());
    let mut stderr = child.stderr.take().unwrap();
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    // Stop it if it hangs.
    let done = kill_after(child.id(), if rebuild { 900 } else { 420 });
    let (mut lookups, mut seen, mut result) = (Vec::new(), std::collections::HashSet::new(), None);
    for line in std::io::BufReader::new(child.stdout.take().unwrap()).lines() {
        let Ok(line) = line else { break };
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let content = v["message"]["content"].as_array().cloned().unwrap_or_default();
        match v["type"].as_str().unwrap_or("") {
            "assistant" => {
                for c in content.iter().filter(|c| c["type"] == "tool_use") {
                    let (name, a) = (c["name"].as_str().unwrap_or(""), &c["input"]);
                    lookups.push(if name.ends_with("read_page") {
                        format!("{} p.{}", a["file"].as_str().unwrap_or(""), a["page"])
                    } else {
                        format!("«{}»", a["query"].as_str().unwrap_or(""))
                    });
                }
            }
            "user" => {
                for c in content.iter().filter(|c| c["type"] == "tool_result") {
                    let text = match &c["content"] {
                        Value::String(s) => s.clone(),
                        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
                        _ => String::new(),
                    };
                    seen.extend(cited_pages(&text));
                }
            }
            "result" => result = Some(v),
            _ => {}
        }
    }
    let _ = child.wait();
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    let err = err_reader.join().unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    fed.map_err(|e| format!("Claude Code: {e}"))?;
    let Some(r) = result else {
        return Err(claude_failed(&err));
    };
    if r["is_error"].as_bool() == Some(true) || r["subtype"] != "success" {
        let why = r["result"].as_str().filter(|s| !s.is_empty()).or(r["subtype"].as_str()).unwrap_or("error");
        return Err(format!("Claude Code: {}", why.chars().take(300).collect::<String>()));
    }
    // modelUsage adds up every turn of the run.
    let (mut inp, mut cached, mut created, mut out, mut cost) = (0u64, 0u64, 0u64, 0u64, 0f64);
    for m in r["modelUsage"].as_object().into_iter().flat_map(|o| o.values()) {
        let n = |k: &str| m[k].as_u64().unwrap_or(0);
        inp += n("inputTokens");
        cached += n("cacheReadInputTokens");
        created += n("cacheCreationInputTokens");
        out += n("outputTokens");
        cost += m["costUSD"].as_f64().unwrap_or(0.0);
    }
    let usage = json!({
        "engine": "claude",
        "model": model,
        "effort": effort,
        "prompt_tokens": inp + cached + created,
        "prompt_cache_hit_tokens": cached,
        "prompt_cache_miss_tokens": inp + created,
        "completion_tokens": out,
        "list_cost_usd": cost,
    });
    Ok(Run { content: r["result"].as_str().unwrap_or("").to_string(), usage, lookups, seen })
}

/// "«file» p.N" mentions in a tool result: the pages the model has actually seen.
fn cited_pages(s: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(a) = rest.find('«') {
        let after = &rest[a + '«'.len_utf8()..];
        let Some(b) = after.find('»') else { break };
        let name = &after[..b];
        let tail = &after[b + '»'.len_utf8()..];
        if let Some(num) = tail.strip_prefix(" p.") {
            let digits: String = num.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse() {
                out.push((name.to_string(), n));
            }
        }
        rest = tail;
    }
    out
}

/// Keeps only references to pages that exist and that the model saw, and writes them at the end
/// of the slide's notes ("Material: «Lab3.pdf» p. 4", in the slides' language).
fn attach_refs(slides: &mut [Value], docs: &[crate::material::Doc], seen: &std::collections::HashSet<(String, usize)>, lang: &str) {
    let label = match lang {
        "it" => "Materiale",
        "fr" => "Support",
        _ => "Material",
    };
    for s in slides.iter_mut() {
        let mut refs: Vec<(String, usize)> = Vec::new();
        for r in s["refs"].as_array().into_iter().flatten() {
            let page = r["page"].as_u64().unwrap_or(0) as usize;
            if let Some(name) = crate::material::page_exists(docs, r["file"].as_str().unwrap_or(""), page) {
                if seen.contains(&(name.clone(), page)) && !refs.contains(&(name.clone(), page)) {
                    refs.push((name, page));
                }
            }
        }
        if let Some(o) = s.as_object_mut() {
            o.remove("refs");
        }
        if refs.is_empty() {
            continue;
        }
        let mut by_file: Vec<(String, Vec<usize>)> = Vec::new();
        for (f, p) in refs.into_iter().take(3) {
            match by_file.iter_mut().find(|x| x.0 == f) {
                Some(x) => x.1.push(p),
                None => by_file.push((f, vec![p])),
            }
        }
        let text = by_file
            .iter()
            .map(|(f, ps)| format!("«{f}» p. {}", ps.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")))
            .collect::<Vec<_>>()
            .join("; ");
        let notes = s["notes"].as_str().unwrap_or("").trim().to_string();
        s["notes"] = json!(if notes.is_empty() { format!("{label}: {text}.") } else { format!("{notes} {label}: {text}.") });
    }
}

/// Marks with "(?)" every command, file, address or number on the slides that can't be found in
/// the transcript (after glossary corrections). Returns how many were marked.
///
/// A word that the transcript only has misheard ("DNS Mask") is accepted when the course
/// material spells it that way ("dnsmasq") and it is close to what was heard.
fn fact_check(slides: &mut [Value], transcript: &str, course: &str, material: &std::collections::HashSet<String>) -> u64 {
    let fixed = crate::glossary::Corrector::new(course).apply(transcript).to_lowercase();
    let g = crate::glossary::load(course);
    let mut hay = fixed.clone();
    for x in g["terms"].as_array().into_iter().flatten() {
        hay.push(' ');
        hay.push_str(&x.as_str().unwrap_or("").to_lowercase());
    }
    let words: std::collections::HashSet<String> = hay.split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() >= 2).map(String::from).collect();
    let digits: String = hay.chars().filter(|c| !matches!(c, ' ' | '.' | ',' | ':' | '\n')).collect();
    // Heard forms, including two words run together ("dns mask" → "dnsmask").
    let toks: Vec<&str> = hay.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let heard: std::collections::HashSet<String> =
        toks.iter().map(|w| w.to_string()).chain(toks.windows(2).map(|w| format!("{}{}", w[0], w[1]))).filter(|w| w.chars().count() >= 4).collect();
    let in_material = |r: &str| {
        let n = r.chars().count();
        n >= 5
            && material.contains(r)
            && heard.iter().any(|w| (w.chars().count() as i64 - n as i64).abs() <= 2 && crate::material::lev(w, r) <= 2)
    };
    let known = |s: &str| -> bool {
        s.split(|c: char| !c.is_alphanumeric()).filter(|r| !r.is_empty()).all(|r| {
            let r = r.to_lowercase();
            if r.chars().all(|c| c.is_ascii_digit()) {
                r.len() < 2 || digits.contains(&r)
            } else if r.chars().count() < 4 || words.contains(&r) || hay.contains(&r) {
                true
            } else {
                (r.chars().count() >= 6 && words.iter().any(|w| (w.len() as i64 - r.len() as i64).abs() <= 1 && edit1(w, &r))) || in_material(&r)
            }
        })
    };
    let mut n = 0;
    // `code` spans in text.
    let mut mark_text = |s: &str| -> String {
        let mut out = String::new();
        let mut rest = s;
        while let Some(a) = rest.find('`') {
            let Some(b) = rest[a + 1..].find('`') else { break };
            let (code, after) = (&rest[a + 1..a + 1 + b], &rest[a + 2 + b..]);
            out.push_str(&rest[..a + 2 + b]);
            if !known(code) && !after.trim_start().starts_with("(?)") {
                out.push_str(" (?)");
                n += 1;
            }
            rest = after;
        }
        out.push_str(rest);
        out
    };
    let visit_text = |v: &mut Value, f: &mut dyn FnMut(&str) -> String| {
        if let Some(s) = v.as_str() {
            *v = json!(f(s));
        }
    };
    let mut flagged_lines = 0;
    for s in slides.iter_mut() {
        for col in ["left", "right"] {
            for b in s[col].as_array_mut().into_iter().flatten().filter(|b| b.is_object()) {
                for k in ["text", "term"] {
                    visit_text(&mut b[k], &mut mark_text);
                }
                for it in b["items"].as_array_mut().into_iter().flatten() {
                    visit_text(it, &mut mark_text);
                    if it.is_object() {
                        for k in ["title", "text"] {
                            visit_text(&mut it[k], &mut mark_text); // timeline steps
                        }
                    }
                }
                if b["type"] == "algo" {
                    for l in b["lines"].as_array_mut().into_iter().flatten() {
                        visit_text(l, &mut mark_text);
                    }
                }
                for row in b["rows"].as_array_mut().into_iter().flatten() {
                    for c in row.as_array_mut().into_iter().flatten() {
                        if c.is_string() {
                            visit_text(c, &mut mark_text);
                        }
                    }
                }
                // Python: identifiers must be traceable; the language's own words don't count.
                if b["type"] == "code" {
                    for l in b["lines"].as_array_mut().into_iter().flatten() {
                        let line = l.as_str().unwrap_or("").to_string();
                        let t = line.split('#').next().unwrap_or("").trim();
                        if t.is_empty() || line.trim_end().ends_with("(?)") {
                            continue;
                        }
                        let rest: Vec<&str> = t.split(|c: char| !c.is_alphanumeric() && c != '_').filter(|w| !w.is_empty() && !PY_WORDS.contains(w)).collect();
                        if !known(&rest.join(" ")) {
                            *l = json!(format!("{line}  (?)"));
                            flagged_lines += 1;
                        }
                    }
                }
                // Terminal: every non-comment line must be traceable to the transcript.
                if b["type"] == "term" {
                    for l in b["lines"].as_array_mut().into_iter().flatten() {
                        let line = l.as_str().unwrap_or("").to_string();
                        let t = line.trim();
                        if t.is_empty() || t.starts_with('#') || t.starts_with('!') || t.ends_with("(?)") {
                            continue;
                        }
                        let cmd = t.strip_prefix("$ ").unwrap_or_else(|| t.split_once("# ").map(|(_, c)| c).unwrap_or(t));
                        if !known(cmd) {
                            *l = json!(format!("{line}  (?)"));
                            flagged_lines += 1;
                        }
                    }
                }
            }
        }
    }
    for s in slides.iter_mut() {
        for c in s["cards"].as_array_mut().into_iter().flatten().filter(|c| c.is_object()) {
            for k in ["q", "a"] {
                visit_text(&mut c[k], &mut mark_text);
            }
        }
    }
    n + flagged_lines
}

/// Python keywords and common builtins: not expected in the transcript.
const PY_WORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from",
    "global", "if", "import", "in", "is", "lambda", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "print", "range", "len",
    "self", "np", "torch", "nn", "plt",
];

/// True if a and b differ by at most one insertion, deletion or substitution.
fn edit1(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (s, l) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if l.len() - s.len() > 1 {
        return false;
    }
    let (mut i, mut j, mut diff) = (0, 0, 0);
    while i < s.len() && j < l.len() {
        if s[i] == l[j] {
            i += 1;
            j += 1;
        } else {
            diff += 1;
            if diff > 1 {
                return false;
            }
            if s.len() == l.len() {
                i += 1;
            }
            j += 1;
        }
    }
    diff + (l.len() - j) + (s.len() - i) <= 1
}

/// Recent transcripts, newest first.
pub fn recent(dir: &Path) -> Vec<Value> {
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy();
            n.ends_with(".md") && !n.contains(" · ")
        })
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter()
        .take(30)
        .map(|(_, p)| json!({ "name": p.file_stem().unwrap_or_default().to_string_lossy(), "path": p.to_string_lossy() }))
        .collect()
}

/// One edit to a finished line, made by hand: `set` replaces line `i`'s text (empty deletes
/// it), `insert` adds a line at index `i` (stamped with the time of the line above it),
/// `delete` removes line `i`. Other parts of the transcript (title, date, "Continued" notes)
/// stay where they are. Returns the new transcript and how the line count changed, or `None`
/// when there's nothing to do.
pub fn edit_body(body: &str, op: &str, i: usize, text: &str) -> Result<Option<(String, i64)>, String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut parts: Vec<String> = body.split("\n\n").map(str::to_string).collect();
    let tail = parts.pop().unwrap_or_default();
    let is_line = |x: &str| x.trim().strip_prefix("**[").and_then(|r| r.split_once("]** ")).is_some();
    let at: Vec<usize> = (0..parts.len()).filter(|&k| is_line(&parts[k])).collect();
    let time = |k: usize| parts[k].trim()[3..].split_once("]** ").map(|(t, _)| t.to_string()).unwrap_or_default();
    let gone = "That line isn't in the transcript any more.";
    let shift = match op {
        "set" | "delete" => {
            let k = *at.get(i).ok_or(gone)?;
            if op == "delete" || text.is_empty() {
                parts.remove(k);
                -1
            } else {
                parts[k] = format!("**[{}]** {text}", time(k));
                0
            }
        }
        "insert" if !text.is_empty() => {
            let (t, k) = match i {
                // Before the first line (or after the header when there are none yet).
                0 => (at.first().map(|&k| time(k)).unwrap_or_else(|| "00:00:00".into()), at.first().copied().unwrap_or(parts.len())),
                _ => {
                    let above = *at.get(i - 1).ok_or(gone)?;
                    (time(above), above + 1)
                }
            };
            parts.insert(k, format!("**[{t}]** {text}"));
            1
        }
        _ => return Ok(None),
    };
    parts.push(tail);
    Ok(Some((parts.join("\n\n"), shift)))
}

/// After a line is added (`shift` 1) or removed (-1) at index `i`, the deck's line ranges move
/// with it, so its slides keep pointing at the same lines.
pub fn shift_deck(p: &Path, i: usize, shift: i64) -> Result<(), String> {
    if shift == 0 || !deck_path(p, ".json").exists() {
        return Ok(());
    }
    let mut deck = load_deck(p);
    for b in deck["batches"].as_array_mut().into_iter().flatten() {
        for key in ["from_line", "to_line"] {
            if let Some(n) = b[key].as_u64().filter(|&n| n as usize > i) {
                b[key] = json!((n as i64 + shift).max(0));
            }
        }
    }
    let (title, _) = read_transcript(p)?;
    save(p, &deck, &title)
}

/// A hand edit to a lecture that isn't being recorded (see `edit_body`), saved to its file.
pub fn edit_line(p: &Path, op: &str, i: usize, text: &str) -> Result<Value, String> {
    let body = std::fs::read_to_string(p).map_err(|e| format!("Can't read the transcript: {e}"))?;
    if let Some((body, shift)) = edit_body(&body, op, i, text)? {
        write_atomic(p, &body)?;
        shift_deck(p, i, shift)?;
    }
    transcript_payload(p)
}

/// For opening a past lecture in the transcript pane.
pub fn transcript_payload(p: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("Can't read the transcript: {e}"))?;
    Ok(json!({
        "type": "transcript",
        "path": p.to_string_lossy(),
        "title": parse_transcript(&text).0,
        "lines": lines_payload(&text),
    }))
}

/// The finished lines as `[[seconds, text], …]`, as the transcript pane takes them.
pub fn lines_payload(text: &str) -> Value {
    let secs = |t: &str| t.split(':').fold(0u64, |a, x| a * 60 + x.parse::<u64>().unwrap_or(0));
    json!(parse_transcript(text).1.iter().map(|(t, x)| json!([secs(t), x])).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every block type the prompt allows, including plain-string list items and a stray
    /// string block, must go through the fact check without panicking.
    #[test]
    fn style_rules_default_to_nothing() {
        assert_eq!(style_rules(&json!({})), "");
        assert_eq!(style_rules(&json!({ "density": "balanced", "titles": "statement", "notes": "short", "visuals": "balanced", "items": "", "rules": " " })), "");
        let r = style_rules(&json!({ "titles": "question", "rules": "- Keep English terms" }));
        assert!(r.contains("STYLE") && r.contains("short question") && r.contains("- Keep English terms") && !r.contains("Density"));
    }

    #[test]
    fn edit_line_keeps_the_file_and_deck_in_step() {
        let dir = std::env::temp_dir().join(format!("tl-edit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("L.md");
        std::fs::write(&p, "# L\n\n_date_\n\n**[00:00:01]** a\n\n**[00:00:05]** b\n\n_Continued_\n\n**[00:01:00]** c\n\n").unwrap();
        std::fs::write(deck_path(&p, ".json"), r#"{"batches":[{"from_line":0,"to_line":2,"slides":[]}]}"#).unwrap();
        let lines = |v: Value| v["lines"].as_array().unwrap().iter().map(|l| format!("{}:{}", l[0], l[1].as_str().unwrap())).collect::<Vec<_>>();
        assert_eq!(lines(edit_line(&p, "set", 1, " b  fixed\n\nnow ").unwrap()), ["1:a", "5:b fixed now", "60:c"]);
        assert_eq!(lines(edit_line(&p, "insert", 1, "mine").unwrap()), ["1:a", "1:mine", "5:b fixed now", "60:c"]);
        assert_eq!(load_deck(&p)["batches"][0]["to_line"], 3);
        assert_eq!(lines(edit_line(&p, "insert", 0, "top").unwrap())[0], "1:top");
        assert_eq!(lines(edit_line(&p, "delete", 4, "").unwrap()), ["1:top", "1:a", "1:mine", "5:b fixed now"]);
        assert_eq!(lines(edit_line(&p, "set", 0, "").unwrap()).len(), 3);
        assert_eq!(load_deck(&p)["batches"][0]["to_line"], 3);
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.starts_with("# L\n\n_date_\n\n") && body.contains("_Continued_") && body.ends_with("\n\n"));
        assert!(edit_line(&p, "set", 9, "x").is_err());
        // A recording's in-memory transcript: header only, then lines appended as they come.
        let (b, n) = edit_body("# L\n\n_date_\n\n", "insert", 0, "first").unwrap().unwrap();
        assert_eq!((b.as_str(), n), ("# L\n\n_date_\n\n**[00:00:00]** first\n\n", 1));
        assert!(edit_body(&b, "insert", 1, "  ").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sample_lecture_parses() {
        assert!(parse_transcript(SAMPLE_LECTURE).1.len() >= 10);
    }

    #[test]
    fn fact_check_handles_all_block_shapes() {
        let mut slides = vec![json!({
            "kind": "content",
            "left": [
                { "type": "list", "items": ["usa `dnsmasq`", "testo"] },
                { "type": "timeline", "items": [{ "when": "1ª", "title": "VAE `xyzzy`" }] },
                { "type": "algo", "lines": ["per ogni `batch`:"] },
                { "type": "code", "lines": ["for x in loader:", "    frobnicate(x)"] },
                "blocco sbagliato"
            ],
            "right": { "type": "p", "text": "solo" }
        }), json!({ "kind": "review", "cards": [{ "q": "Cos'è `xyzzy`?", "a": "boh" }, "carta sbagliata"] })];
        let flagged = fact_check(&mut slides, "[00:00:01] usiamo dnsmasq con il loader e il batch\n", "", &Default::default());
        assert!(flagged >= 3, "flagged {flagged}");
        assert!(slides[0]["left"][3]["lines"][1].as_str().unwrap().ends_with("(?)"));
        assert!(!slides[0]["left"][3]["lines"][0].as_str().unwrap().ends_with("(?)"));
    }
}
