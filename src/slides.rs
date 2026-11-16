//! Slides: DeepSeek turns the transcript into a Protocolli & Reti deck, on demand.
//!
//! The deck lives next to the transcript as "<lecture> · Slide.json" (one batch per click, so
//! the last click can be undone) and is exported to "<lecture> · Slide.html" after every change.

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const TOKENS_CSS: &str = include_str!("ds/tokens.css");
pub const BUNDLE_CSS: &str = include_str!("ds/bundle.css");
pub const SLIDES_CSS: &str = include_str!("slides.css");
pub const SLIDES_JS: &str = include_str!("slides.js");
const DECK_HTML: &str = include_str!("deck.html");
const PROMPT: &str = include_str!("slides_prompt.txt");
const PROMPT_MATERIAL: &str = include_str!("slides_prompt_material.txt");
/// Rounds of material lookups before DeepSeek must answer.
const MAX_TOOL_ROUNDS: usize = 3;

const API_URL: &str = "https://api.deepseek.com/chat/completions";
const MODEL: &str = "deepseek-flash"; // DeepSeek V4.1 Flash
const EFFORT: &str = "low";
const KEYCHAIN_SERVICE: &str = "Lecture Scribe DeepSeek";
const SUFFIX: &str = " · Slide";

pub fn deck_path(transcript: &Path, ext: &str) -> PathBuf {
    let stem = transcript.file_stem().unwrap_or_default().to_string_lossy();
    transcript.with_file_name(format!("{stem}{SUFFIX}{ext}"))
}

/// Title and finished lines `(hh:mm:ss, text)`. The live partial line is the text after the
/// last blank line, so it is never included.
pub fn parse_transcript(text: &str) -> (String, Vec<(String, String)>) {
    let title = text.lines().next().and_then(|l| l.strip_prefix("# ")).unwrap_or("Lezione").trim().to_string();
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

fn read_transcript(p: &Path) -> Result<(String, Vec<(String, String)>), String> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("Non riesco a leggere la trascrizione: {e}"))?;
    Ok(parse_transcript(&text))
}

fn load_deck(t: &Path) -> Value {
    std::fs::read_to_string(deck_path(t, ".json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .filter(|v: &Value| v["batches"].is_array())
        .unwrap_or_else(|| json!({ "batches": [] }))
}

fn slides_of(deck: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for b in deck["batches"].as_array().into_iter().flatten() {
        if b["rebuild"].as_bool() == Some(true) {
            out.clear();
        }
        if b["replace_last"].as_bool() == Some(true) {
            out.pop();
        }
        out.extend(b["slides"].as_array().cloned().unwrap_or_default());
    }
    out
}

fn done_of(deck: &Value) -> usize {
    deck["batches"].as_array().and_then(|b| b.last()).and_then(|b| b["to_line"].as_u64()).unwrap_or(0) as usize
}

fn write_atomic(p: &Path, s: &str) -> Result<(), String> {
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, s).and_then(|_| std::fs::rename(&tmp, p)).map_err(|e| format!("Salvataggio non riuscito: {e}"))
}

pub fn foot(course: &str, title: &str) -> String {
    [course.trim(), title.trim()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" · ")
}

/// The transcript's title, or DeepSeek's suggestion when the lecture wasn't given one.
fn display_title(deck: &Value, title: &str) -> String {
    let generic = matches!(title.trim().to_lowercase().as_str(), "" | "lecture" | "lezione");
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
        .replace("{{TITLE}}", &esc(&format!("{title} · Slide")))
        .replace("{{BUNDLE}}", BUNDLE_CSS)
        .replace("{{TOKENS}}", TOKENS_CSS)
        .replace("{{SLIDES_CSS}}", SLIDES_CSS)
        .replace("{{SLIDES_JS}}", SLIDES_JS)
        .replace("{{FOOT}}", &js_json(&json!(foot(course, &title))))
        .replace("{{DATA}}", &js_json(&Value::Array(slides)));
    write_atomic(&deck_path(t, ".html"), &html)
}

/// Token counts and an estimated cost (DeepSeek Flash prices; half price off-peak).
fn usage_summary(u: &Value) -> Value {
    use chrono::{Datelike, Timelike};
    let n = |k: &str| u[k].as_u64().unwrap_or(0) as f64;
    let (hit, miss, out) = (n("prompt_cache_hit_tokens"), n("prompt_cache_miss_tokens"), n("completion_tokens"));
    let now = chrono::Utc::now();
    let peak = now.weekday().num_days_from_monday() < 5 && matches!(now.hour(), 1..=3 | 6..=9);
    let k = if peak { 1.0 } else { 0.5 };
    let cost = k * (hit * 0.006 + miss * 0.30 + out * 1.20) / 1e6;
    json!({ "prompt": hit + miss, "cached": hit, "output": out, "cost": cost, "cacheRate": if hit + miss > 0.0 { hit / (hit + miss) } else { 0.0 } })
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
        "html": deck_path(t, ".html").to_string_lossy(),
        "usage": last.map(|b| usage_summary(&b["usage"])),
        "flagged": last.and_then(|b| b["flagged"].as_u64()).unwrap_or(0),
        "lookups": last.map(|b| b["lookups"].clone()).unwrap_or(json!([])),
    }))
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

/// Sends the transcript to DeepSeek and adds the new slides. With `rebuild`, the whole deck is
/// made again from the full lecture (at higher reasoning effort); undo brings the old one back.
///
/// Prompt caching: DeepSeek caches request prefixes automatically. The request is laid out from
/// most stable to least stable: system prompt (fixed per course) → the full transcript so far
/// (append-only, so the previous call's transcript is a prefix of this one's) → glossary, deck
/// state and the instruction (change every call). Repeated clicks re-read the earlier lecture
/// from cache at ~1/50 of the price. The course material's index is part of the system prompt;
/// DeepSeek reads pages through tools, and each tool round only appends to the conversation,
/// so it is cached too.
pub fn generate(t: &Path, course: &str, rebuild: bool) -> Result<Value, String> {
    let (title, lines) = read_transcript(t)?;
    let mut deck = load_deck(t);
    let done = if rebuild { 0 } else { done_of(&deck).min(lines.len()) };
    if lines.len() <= done {
        return Err(if lines.is_empty() { "La trascrizione è vuota.".into() } else { "Nessuna frase nuova dall'ultima volta.".into() });
    }
    let prev = if rebuild { Vec::new() } else { slides_of(&deck) };

    let transcript: String = lines.iter().map(|(t, x)| format!("[{t}] {x}\n")).collect();
    let mut tail = Vec::new();
    let g = crate::glossary::load(course);
    if g["corrections"].as_array().is_some_and(|a| !a.is_empty()) || g["terms"].as_array().is_some_and(|a| !a.is_empty()) {
        tail.push(format!("GLOSSARIO DEL CORSO (già noto, non riproporlo):\n{}", json!({ "corrections": g["corrections"], "terms": g["terms"] })));
    }
    if !prev.is_empty() {
        let titles: Vec<String> = prev.iter().map(|s| format!("- {}", s["title"].as_str().unwrap_or(""))).collect();
        tail.push(format!("TITOLI DELLE SLIDE GIÀ FATTE:\n{}", titles.join("\n")));
        tail.push(format!("ULTIMA SLIDE (JSON):\n{}", prev.last().unwrap()));
    }
    tail.push(if rebuild {
        format!("RICOSTRUZIONE: crea da capo la presentazione dell'intera lezione (righe da [{}] a [{}]).", lines[0].0, lines.last().unwrap().0)
    } else {
        format!(
            "RIGHE NUOVE: da [{}] a [{}] ({} righe). Fai slide solo per queste.",
            lines[done].0,
            lines.last().unwrap().0,
            lines.len() - done
        )
    });
    let user = format!("TRASCRIZIONE DELLA LEZIONE:\n{transcript}\n=== RICHIESTA ===\n{}", tail.join("\n\n"));
    let course_name = if course.trim().is_empty() { "universitario" } else { course.trim() };
    let docs = crate::material::load(course);
    let mut system = PROMPT.replace("{course}", course_name);
    if !docs.is_empty() {
        system.push_str("\n");
        system.push_str(PROMPT_MATERIAL);
        system.push_str("\nINDICE DEL MATERIALE (file, pagina, prima riga):\n");
        system.push_str(&crate::material::index(&docs));
    }
    let mut messages = vec![json!({ "role": "system", "content": system }), json!({ "role": "user", "content": user })];
    let mut body = json!({
        "model": MODEL,
        "thinking": { "type": "enabled" },
        "reasoning_effort": if rebuild { "high" } else { EFFORT },
        "response_format": { "type": "json_object" },
        "max_tokens": if rebuild { 48000 } else { 16000 },
        "stream": false,
    });
    if !docs.is_empty() {
        body["tools"] = crate::material::tools();
    }
    // Tool loop: DeepSeek searches or reads the material, then answers with the slides.
    let mut usage = json!({});
    let mut lookups: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<(String, usize)> = std::collections::HashSet::new();
    let mut round = 0;
    let content = loop {
        if round == MAX_TOOL_ROUNDS && !docs.is_empty() {
            body["tool_choice"] = json!("none");
            messages.push(json!({ "role": "user", "content": "Ricerche concluse: rispondi ora con il JSON delle slide." }));
        }
        body["messages"] = json!(messages);
        let resp = call(&body)?;
        add_usage(&mut usage, &resp["usage"]);
        let msg = resp["choices"][0]["message"].clone();
        let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
        if calls.is_empty() || docs.is_empty() || round >= MAX_TOOL_ROUNDS {
            break msg["content"].as_str().unwrap_or("").to_string();
        }
        messages.push(msg); // with its reasoning_content, as thinking mode requires
        for c in &calls {
            let (name, args) = (c["function"]["name"].as_str().unwrap_or(""), c["function"]["arguments"].as_str().unwrap_or("{}"));
            let result = crate::material::run_tool(&docs, name, args);
            seen.extend(cited_pages(&result));
            let a: Value = serde_json::from_str(args).unwrap_or(json!({}));
            lookups.push(match name {
                "read_page" => format!("{} p.{}", a["file"].as_str().unwrap_or(""), a["page"]),
                _ => format!("«{}»", a["query"].as_str().unwrap_or("")),
            });
            messages.push(json!({ "role": "tool", "tool_call_id": c["id"], "content": result }));
        }
        round += 1;
    };
    let content = content.as_str();
    let (a, b) = (content.find('{'), content.rfind('}'));
    let out: Value = match (a, b) {
        (Some(a), Some(b)) if b > a => serde_json::from_str(&content[a..=b]).map_err(|e| format!("JSON di DeepSeek non valido: {e}"))?,
        _ => return Err("Risposta di DeepSeek senza JSON.".into()),
    };
    let mut slides: Vec<Value> = out["slides"].as_array().cloned().unwrap_or_default().into_iter().filter(|s| s.is_object()).collect();
    let replace = !rebuild && out["replace_last"].is_object() && !prev.is_empty();
    if replace {
        slides.insert(0, out["replace_last"].clone());
    }
    let flagged = fact_check(&mut slides, &transcript, course, &crate::material::words(&docs));
    attach_refs(&mut slides, &docs, &seen);
    let added = slides.len();
    deck["course"] = json!(course.trim());
    if let Some(lt) = out["lecture_title"].as_str().filter(|s| !s.trim().is_empty()) {
        if rebuild || deck["lecture_title"].as_str().is_none() {
            deck["lecture_title"] = json!(lt.trim());
        }
    }
    deck["batches"].as_array_mut().unwrap().push(json!({
        "from_line": done,
        "to_line": lines.len(),
        "replace_last": replace,
        "rebuild": rebuild,
        "slides": slides,
        "flagged": flagged,
        "lookups": lookups,
        "usage": usage,
    }));
    save(t, &deck, &title)?;
    let (nc, nt) = crate::glossary::merge(course, &out["glossary"]);
    let mut st = state(t)?;
    st["learned"] = json!({ "corrections": nc, "terms": nt });
    st["added"] = json!(added);
    st["replaced"] = json!(replace);
    st["rebuilt"] = json!(rebuild);
    Ok(st)
}

fn add_usage(total: &mut Value, u: &Value) {
    for k in ["prompt_tokens", "completion_tokens", "prompt_cache_hit_tokens", "prompt_cache_miss_tokens"] {
        total[k] = json!(total[k].as_u64().unwrap_or(0) + u[k].as_u64().unwrap_or(0));
    }
}

/// "«file» p.N" mentions in a tool result: the pages DeepSeek has actually seen.
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

/// Keeps only references to pages that exist and that DeepSeek saw, and writes them at the end
/// of the slide's notes ("Materiale: «Lab3.pdf» p. 4").
fn attach_refs(slides: &mut [Value], docs: &[crate::material::Doc], seen: &std::collections::HashSet<(String, usize)>) {
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
        s["notes"] = json!(if notes.is_empty() { format!("Materiale: {text}.") } else { format!("{notes} Materiale: {text}.") });
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
            for b in s[col].as_array_mut().into_iter().flatten() {
                for k in ["text", "term"] {
                    visit_text(&mut b[k], &mut mark_text);
                }
                for it in b["items"].as_array_mut().into_iter().flatten() {
                    visit_text(it, &mut mark_text);
                }
                for row in b["rows"].as_array_mut().into_iter().flatten() {
                    for c in row.as_array_mut().into_iter().flatten() {
                        if c.is_string() {
                            visit_text(c, &mut mark_text);
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
    n + flagged_lines
}

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

fn api_key() -> Result<String, String> {
    if let Ok(k) = std::env::var("DEEPSEEK_API_KEY") {
        return Ok(k);
    }
    let out = Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-a", "deepseek", "-w"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("Chiave DeepSeek non trovata nel Portachiavi (servizio «Lecture Scribe DeepSeek»).".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// POST with the system curl. The key goes in through stdin, so it never shows up in `ps`.
fn call(body: &Value) -> Result<Value, String> {
    let key = api_key()?;
    let tmp = std::env::temp_dir().join(format!("lecture-scribe-{}-{}.json", std::process::id(), chrono::Local::now().timestamp_millis()));
    std::fs::write(&tmp, body.to_string()).map_err(|e| e.to_string())?;
    let child = Command::new("curl")
        .args(["-sS", "--max-time", "300", "-X", "POST", API_URL, "-H", "@-", "--data-binary"])
        .arg(format!("@{}", tmp.display()))
        .args(["-w", "\n%{http_code}"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let result = (|| {
        let mut child = child.map_err(|e| e.to_string())?;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("Authorization: Bearer {key}\nContent-Type: application/json\n").as_bytes())
            .map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        if !out.status.success() {
            return Err(format!("DeepSeek non raggiungibile: {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        let (resp, code) = text.rsplit_once('\n').unwrap_or((&text, "0"));
        if code.trim() != "200" {
            return Err(format!("DeepSeek {}: {}", code.trim(), resp.chars().take(300).collect::<String>()));
        }
        serde_json::from_str(resp).map_err(|e| format!("Risposta di DeepSeek illeggibile: {e}"))
    })();
    let _ = std::fs::remove_file(&tmp);
    result
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

/// For opening a past lecture in the transcript pane.
pub fn transcript_payload(p: &Path) -> Result<Value, String> {
    let (title, lines) = read_transcript(p)?;
    let secs = |t: &str| t.split(':').fold(0u64, |a, x| a * 60 + x.parse::<u64>().unwrap_or(0));
    Ok(json!({
        "type": "transcript",
        "path": p.to_string_lossy(),
        "title": title,
        "lines": lines.iter().map(|(t, x)| json!([secs(t), x])).collect::<Vec<_>>(),
    }))
}
