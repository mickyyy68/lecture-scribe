//! Docs: the student's own documents about a lecture (notes, summaries, exercises).
//!
//! They live next to the transcript in "<lecture> · Docs/", one Markdown file per doc, with
//! pasted images in "<lecture> · Docs/images/". Plain Markdown, so any other editor opens them:
//! GitHub tables, `$$` formulas, ```` ```chart ```` blocks holding one slide diagram block as JSON
//! (drawn by slides.js), and time links `[12:40](#t=00:12:40)` back to the lecture.
//!
//! `handle` answers the page's `doc*` commands. `draft` asks Claude Code for notes of the whole
//! lecture and saves them as a new doc, named by Claude.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const PROMPT: &str = include_str!("docs_prompt.txt");
const SUFFIX: &str = " · Docs";
/// The diagram blocks a ```chart block may hold: the slides' own (see slides_prompt.txt).
pub const CHART_TYPES: &[&str] = &["seq", "net", "graph", "plot", "bars", "sketch", "timeline", "arch", "stack", "header"];

/// "<lecture> · Docs/" beside the transcript.
pub fn dir(t: &Path) -> PathBuf {
    let stem = t.file_stem().unwrap_or_default().to_string_lossy();
    t.with_file_name(format!("{stem}{SUFFIX}"))
}

/// A file name from a doc title: no path separators or control characters, at most 80 chars.
pub fn safe_name(s: &str) -> String {
    let s: String = s.chars().map(|c| if matches!(c, '/' | ':' | '\\') || c.is_control() { ' ' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s: String = s.trim_start_matches('.').chars().take(80).collect();
    if s.trim().is_empty() { "Untitled".into() } else { s.trim().to_string() }
}

/// `dir/<name>.md`, or "<name> 2.md", "<name> 3.md"… when taken (`except` may keep its name).
fn free_path(dir: &Path, name: &str, except: Option<&Path>) -> PathBuf {
    let name = safe_name(name);
    (1..)
        .map(|n| dir.join(if n == 1 { format!("{name}.md") } else { format!("{name} {n}.md") }))
        .find(|p| !p.exists() || Some(p.as_path()) == except)
        .unwrap()
}

/// The lecture's docs, oldest first (so the list doesn't reorder as they're edited).
pub fn list(t: &Path) -> Vec<Value> {
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir(t))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .filter_map(|p| {
            let m = p.metadata().ok()?;
            Some((m.created().or_else(|_| m.modified()).ok()?, p))
        })
        .collect();
    v.sort();
    v.into_iter().map(|(_, p)| json!({ "name": p.file_stem().unwrap_or_default().to_string_lossy(), "path": p.to_string_lossy() })).collect()
}

fn write_atomic(p: &Path, s: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't save: {e}"))?;
    }
    let tmp = p.with_extension("md.tmp");
    std::fs::write(&tmp, s).and_then(|_| std::fs::rename(&tmp, p)).map_err(|e| format!("Couldn't save: {e}"))
}

/// A new doc in the lecture's folder. Returns its path.
pub fn create(t: &Path, name: &str, md: &str) -> Result<PathBuf, String> {
    let p = free_path(&dir(t), name, None);
    write_atomic(&p, md.as_bytes())?;
    Ok(p)
}

/// Renames a doc (same folder). Returns the new path.
pub fn rename(doc: &Path, name: &str) -> Result<PathBuf, String> {
    let p = free_path(doc.parent().ok_or("No folder")?, name, Some(doc));
    if p != doc {
        std::fs::rename(doc, &p).map_err(|e| format!("Couldn't rename: {e}"))?;
    }
    Ok(p)
}

/// Moves a file to the macOS Trash, so it can be put back.
pub fn trash(p: &Path) -> Result<(), String> {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2_foundation::{NSString, NSURL};
    if !p.exists() {
        return Ok(());
    }
    let ok = objc2::rc::autoreleasepool(|_| unsafe {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy()));
        let Some(cls) = AnyClass::get(c"NSFileManager") else { return false };
        let fm: *mut AnyObject = msg_send![cls, defaultManager];
        let null: *mut *mut AnyObject = std::ptr::null_mut();
        let ok: Bool = msg_send![fm, trashItemAtURL: &*url, resultingItemURL: null, error: null];
        ok.as_bool()
    });
    if ok { Ok(()) } else { Err("Couldn't move the doc to the Trash.".into()) }
}

/// The relative `images/…` paths a doc's Markdown points at.
fn image_refs(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = md;
    while let Some(a) = rest.find("](images/") {
        let tail = &rest[a + 2..];
        let end = tail.find(|c: char| c == ')' || c.is_whitespace()).unwrap_or(tail.len());
        out.push(tail[..end].to_string());
        rest = &tail[end..];
    }
    out
}

fn mime(p: &str) -> &'static str {
    match p.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    }
}

fn data_url(file: &Path, rel: &str) -> Option<String> {
    Some(format!("data:{};base64,{}", mime(rel), b64_encode(&std::fs::read(file).ok()?)))
}

/// A doc for the editor: its Markdown, and its images as data URLs (the page can't read files).
pub fn read(doc: &Path) -> Result<Value, String> {
    let md = std::fs::read_to_string(doc).map_err(|e| format!("Can't read the doc: {e}"))?;
    let base = doc.parent().unwrap_or(Path::new("."));
    let images: serde_json::Map<String, Value> = image_refs(&md)
        .into_iter()
        .filter(|r| !r.contains(".."))
        .filter_map(|r| Some((r.clone(), json!(data_url(&base.join(r.replace("%20", " ")), &r)?))))
        .collect();
    Ok(json!({ "type": "doc", "doc": doc.to_string_lossy(), "name": doc.file_stem().unwrap_or_default().to_string_lossy(), "md": md, "images": images }))
}

/// Saves a pasted or dropped image into the doc's `images/` folder. Returns its relative path.
pub fn save_image(doc: &Path, data: &str, ext: &str) -> Result<String, String> {
    let ext = match ext.to_lowercase().as_str() {
        e @ ("png" | "jpg" | "jpeg" | "gif" | "webp") => e.to_string(),
        _ => "png".into(),
    };
    let bytes = b64_decode(data.rsplit(',').next().unwrap_or(data)).ok_or("The image couldn't be read.")?;
    let images = doc.parent().ok_or("No folder")?.join("images");
    std::fs::create_dir_all(&images).map_err(|e| e.to_string())?;
    let stamp = chrono::Local::now().format("%Y-%m-%d %H.%M.%S");
    let name = (1..).map(|n| if n == 1 { format!("{stamp}.{ext}") } else { format!("{stamp} {n}.{ext}") }).find(|n| !images.join(n).exists()).unwrap();
    std::fs::write(images.join(&name), bytes).map_err(|e| format!("Couldn't save the image: {e}"))?;
    Ok(format!("images/{}", name.replace(' ', "%20")))
}

/// Copies an image file chosen in the file picker into the doc's `images/` folder.
pub fn add_image_file(doc: &Path, file: &Path) -> Result<String, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("Can't read the image: {e}"))?;
    let ext = file.extension().unwrap_or_default().to_string_lossy().to_string();
    save_image(doc, &b64_encode(&bytes), &ext)
}

/// The `docImage` event for an image saved at `src` (relative to the doc).
pub fn image_event(t: &Path, doc: &Path, id: &Value, src: &str) -> Value {
    let url = data_url(&doc.parent().unwrap_or(Path::new(".")).join(src.replace("%20", " ")), src);
    json!({ "type": "docImage", "path": t.to_string_lossy(), "doc": doc.to_string_lossy(), "id": id, "src": src, "url": url })
}

/// Answers the page's doc commands (all but `docDraft`). `path` is the lecture, `doc` a doc.
pub fn handle(msg: &Value) -> Value {
    let s = |k: &str| msg[k].as_str().unwrap_or("").to_string();
    let (t, doc) = (PathBuf::from(s("path")), PathBuf::from(s("doc")));
    if msg["cmd"] == "docPickImage" && inside(&t, &doc) {
        return match pick_image().map(|f| add_image_file(&doc, &f)) {
            Some(Ok(src)) => image_event(&t, &doc, &msg["id"], &src),
            Some(Err(e)) => json!({ "type": "docError", "path": t.to_string_lossy(), "message": e }),
            None => json!({ "type": "docImage", "path": t.to_string_lossy(), "doc": doc.to_string_lossy(), "id": msg["id"], "cancelled": true }),
        };
    }
    handle_files(msg)
}

fn inside(t: &Path, doc: &Path) -> bool {
    !doc.as_os_str().is_empty() && doc.parent() == Some(dir(t).as_path())
}

/// The macOS file picker for one image (AppleScript, no extra dependencies). None if cancelled.
fn pick_image() -> Option<PathBuf> {
    let out = std::process::Command::new("osascript")
        .args(["-e", r#"POSIX path of (choose file with prompt "Image for the doc" of type {"public.image"})"#])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!p.is_empty()).then(|| PathBuf::from(p))
}

fn handle_files(msg: &Value) -> Value {
    let s = |k: &str| msg[k].as_str().unwrap_or("").to_string();
    let (t, doc) = (PathBuf::from(s("path")), PathBuf::from(s("doc")));
    let listing = |open: Option<&Path>| json!({ "type": "docs", "path": t.to_string_lossy(), "docs": list(&t), "open": open.map(|p| p.to_string_lossy()) });
    // A doc command must stay inside the lecture's own folder.
    let inside = inside(&t, &doc);
    let r: Result<Value, String> = match msg["cmd"].as_str().unwrap_or("") {
        "docsList" => Ok(listing(None)),
        "docRead" if inside => read(&doc).map(|mut v| {
            v["path"] = json!(t.to_string_lossy());
            v
        }),
        "docSave" if inside => write_atomic(&doc, s("md").as_bytes()).map(|_| json!({ "type": "docSaved", "path": t.to_string_lossy(), "doc": doc.to_string_lossy(), "seq": msg["seq"] })),
        "docCreate" => create(&t, &s("name"), &s("md")).map(|p| listing(Some(&p))),
        "docRename" if inside => rename(&doc, &s("name")).map(|p| listing(Some(&p))),
        "docDelete" if inside => trash(&doc).map(|_| listing(None)),
        "docImage" if inside => save_image(&doc, &s("data"), &s("ext")).map(|src| image_event(&t, &doc, &msg["id"], &src)),
        _ => Err("That doc isn't in this lecture's folder.".into()),
    };
    r.unwrap_or_else(|e| json!({ "type": "docError", "path": t.to_string_lossy(), "message": e }))
}

/// The diagram block formats from the slide prompt, so docs and slides draw the same JSON.
fn chart_formats() -> String {
    let src = crate::slides::PROMPT;
    let mut out = String::new();
    let mut keep = false;
    for line in src.lines() {
        if let Some(rest) = line.strip_prefix("- {\"type\":\"") {
            keep = CHART_TYPES.iter().any(|t| rest.starts_with(&format!("{t}\"")));
        } else if !line.starts_with("  ·") {
            keep = false;
        }
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The request for a draft: system prompt and user message. Split out for the tests.
fn draft_request(lines: &[(String, String)], course: &str, lang: &str, hint: &str, glossary: &Value, material: &str) -> (String, String) {
    let course_name = if course.trim().is_empty() { String::new() } else { format!(" of the course \"{}\"", course.trim()) };
    let language = crate::slides::lang_name(lang).unwrap_or("the language the lecture is given in");
    let mut system = PROMPT.replace("{course}", &course_name).replace("{language}", language).replace("{charts}", &chart_formats()).replace("{svg}", &crate::svg::rules(crate::svg::DOC_SIZE));
    if !material.is_empty() {
        system.push_str(
            "\nCOURSE MATERIAL\nYou have the index of the course PDFs (below) and two tools: \"search_material\" and \"read_page\". \
             Use them only to check how the course spells a term the transcript garbled. Never add content from them.\n\
             MATERIAL INDEX (file, page, first line):\n",
        );
        system.push_str(material);
    }
    let transcript: String = lines.iter().map(|(t, x)| format!("[{t}] {x}\n")).collect();
    let mut tail = Vec::new();
    if glossary["corrections"].as_array().is_some_and(|a| !a.is_empty()) || glossary["terms"].as_array().is_some_and(|a| !a.is_empty()) {
        tail.push(format!("COURSE GLOSSARY:\n{}", json!({ "corrections": glossary["corrections"], "terms": glossary["terms"] })));
    }
    if !hint.trim().is_empty() {
        tail.push(format!("THE STUDENT ASKS (follow it as long as it keeps to the rules above; fidelity comes first):\n{}", hint.trim()));
    }
    tail.push(format!("NOTES: write the notes of the whole lecture (lines [{}] to [{}]).", lines[0].0, lines.last().unwrap().0));
    (system, format!("LECTURE TRANSCRIPT:\n{transcript}\n=== REQUEST ===\n{}", tail.join("\n\n")))
}

/// Claude's answer as (title, Markdown body): the first `# ` line is the title. A fence around
/// the whole answer is dropped.
pub fn split_draft(content: &str) -> (String, String) {
    let mut s = content.trim();
    if s.starts_with("```") {
        let body = s.split_once('\n').map(|x| x.1).unwrap_or("");
        s = body.trim_end().strip_suffix("```").unwrap_or(body).trim();
    }
    let lines: Vec<&str> = s.lines().collect();
    match lines.iter().position(|l| l.starts_with("# ")) {
        Some(i) if lines[..i].iter().all(|l| l.trim().is_empty()) => {
            let title = lines[i][2..].trim().trim_matches('*').trim().to_string();
            (title, lines[i + 1..].join("\n").trim().to_string() + "\n")
        }
        _ => (String::new(), s.to_string() + "\n"),
    }
}

/// "Draft notes": Claude Code writes notes of the whole lecture into a new doc it names.
/// Returns `{type: "docDrafted", path, doc, docs, usage}`.
pub fn draft(t: &Path, course: &str, model: &str, effort: &str, lang: &str, hint: &str) -> Result<Value, String> {
    let (title, lines) = crate::slides::read_transcript(t)?;
    if lines.is_empty() {
        return Err("The transcript is empty.".into());
    }
    let material = crate::material::load(course);
    let index = if material.is_empty() { String::new() } else { crate::material::index(&material) };
    let (system, user) = draft_request(&lines, course, lang, hint, &crate::glossary::load(course), &index);
    // Notes of a whole lecture need care, like Rebuild.
    let (model, mut effort) = crate::slides::model_effort(model, effort);
    if matches!(effort, "low" | "medium") {
        effort = "high";
    }
    let run = crate::slides::run_claude(&system, &user, course, !material.is_empty(), model, effort, true)?;
    let (name, md) = split_draft(&run.content);
    let md = crate::svg::check_markdown(&md, &mut |a| crate::svg::fix_with_claude(a, model, "low"))?;
    if md.trim().is_empty() {
        return Err("Claude's answer was empty.".into());
    }
    let name = if name.is_empty() { format!("Notes · {title}") } else { name };
    let p = create(t, &name, &md)?;
    Ok(json!({
        "type": "docDrafted",
        "path": t.to_string_lossy(),
        "doc": p.to_string_lossy(),
        "docs": list(t),
        "usage": crate::slides::usage_summary(&run.usage),
        "lookups": run.lookups,
    }))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            s.push(if k <= c.len() { B64[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = acc << 6 | B64.iter().position(|&x| x == c)? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lecture(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lecture-scribe-docs-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let t = d.join("2026-11-17 10.00 DNS.md");
        std::fs::write(&t, "# DNS\n\n**[00:00:01]** hello\n\n").unwrap();
        t
    }

    #[test]
    fn docs_live_beside_the_lecture() {
        let t = lecture("files");
        let cmd = |v: Value| handle(&v);
        let path = t.to_string_lossy().to_string();
        let r = cmd(json!({ "cmd": "docCreate", "path": path, "name": "Notes: part 1/2", "md": "Hello\n" }));
        let a = r["open"].as_str().unwrap().to_string();
        assert!(a.ends_with("2026-11-17 10.00 DNS · Docs/Notes part 1 2.md"), "{a}");
        let r = cmd(json!({ "cmd": "docCreate", "path": path, "name": "Notes part 1 2", "md": "" }));
        assert!(r["open"].as_str().unwrap().ends_with("Notes part 1 2 2.md"), "{r}");
        assert_eq!(r["docs"].as_array().unwrap().len(), 2);
        // Save, read back, rename, and nothing outside the folder.
        cmd(json!({ "cmd": "docSave", "path": path, "doc": a, "md": "# x\n\n![](images/a.png)\n" }));
        let png = dir(&t).join("images/a.png");
        std::fs::create_dir_all(png.parent().unwrap()).unwrap();
        std::fs::write(&png, [137, 80, 78, 71]).unwrap();
        let r = cmd(json!({ "cmd": "docRead", "path": path, "doc": a }));
        assert_eq!(r["md"], "# x\n\n![](images/a.png)\n");
        assert_eq!(r["images"]["images/a.png"], "data:image/png;base64,iVBORw==");
        let r = cmd(json!({ "cmd": "docRename", "path": path, "doc": a, "name": "Summary" }));
        assert!(r["open"].as_str().unwrap().ends_with("Summary.md"));
        assert!(!Path::new(&a).exists());
        let r = cmd(json!({ "cmd": "docRead", "path": path, "doc": t.to_string_lossy() }));
        assert_eq!(r["type"], "docError", "the transcript itself isn't a doc");
        let r = cmd(json!({ "cmd": "docSave", "path": path, "doc": "/tmp/elsewhere.md", "md": "x" }));
        assert_eq!(r["type"], "docError");
        // An image is saved and comes back as a data URL.
        let summary = dir(&t).join("Summary.md");
        let r = cmd(json!({ "cmd": "docImage", "path": path, "doc": summary.to_string_lossy(), "id": 7, "data": "data:image/png;base64,iVBORw==", "ext": "png" }));
        assert!(r["src"].as_str().unwrap().starts_with("images/"), "{r}");
        assert_eq!(r["url"], "data:image/png;base64,iVBORw==");
        // The lecture list doesn't show the docs folder.
        assert_eq!(crate::slides::recent(t.parent().unwrap()).len(), 1);
        let _ = std::fs::remove_dir_all(t.parent().unwrap());
    }

    #[test]
    fn delete_moves_to_the_trash() {
        let t = lecture("trash");
        let name = format!("lecture-scribe-test-{}", std::process::id());
        let p = create(&t, &name, "bye\n").unwrap();
        let r = handle(&json!({ "cmd": "docDelete", "path": t.to_string_lossy(), "doc": p.to_string_lossy() }));
        assert_eq!(r["type"], "docs", "{r}");
        assert!(!p.exists());
        let _ = std::fs::remove_file(dirs::home_dir().unwrap().join(format!(".Trash/{name}.md")));
        let _ = std::fs::remove_dir_all(t.parent().unwrap());
    }

    #[test]
    fn base64_round_trips() {
        for b in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0, 255, 128, 7, 9]] {
            assert_eq!(b64_decode(&b64_encode(b)).unwrap(), b);
        }
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_encode(b"Ma"), "TWE=");
    }

    #[test]
    fn draft_answer_and_request() {
        assert_eq!(split_draft("# DNS resolution\n\n## Root servers\ntext"), ("DNS resolution".into(), "## Root servers\ntext\n".into()));
        assert_eq!(split_draft("```markdown\n# Title\nbody\n```"), ("Title".into(), "body\n".into()));
        assert_eq!(split_draft("Some text\n# Late"), (String::new(), "Some text\n# Late\n".into()));
        let lines = vec![("00:00:01".to_string(), "hello".to_string()), ("00:01:00".to_string(), "bye".to_string())];
        let (system, user) = draft_request(&lines, "Networks", "it", "focus on the lab", &json!({}), "");
        assert!(system.contains("Italian") && system.contains("\"Networks\""));
        assert!(system.contains("{\"type\":\"seq\"") && system.contains("· {\"shape\":\"cloud\"") && !system.contains("{\"type\":\"list\""), "chart formats");
        assert!(!system.contains("{charts}"));
        // Drawings: the fenced form with its rules sized for a doc, never the slides' JSON block.
        assert!(system.contains("language \"svg\"") && system.contains("SVG DRAWINGS") && system.contains("W is 720"));
        assert!(!system.contains("{\"type\":\"svg\"") && !system.contains("{svg}") && !system.contains("{size}"));
        // The transcript comes first (prompt caching), the student's request after it.
        let (a, b) = (user.find("[00:00:01] hello").unwrap(), user.find("focus on the lab").unwrap());
        assert!(a < b && user.ends_with("lines [00:00:01] to [00:01:00])."));
    }
}
