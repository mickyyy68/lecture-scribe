//! Versions of a transcript: clean versions Claude made (see clean.rs) beside the original.
//!
//! The original `<lecture>.md` is never touched here: the engine keeps recording into it, and it
//! stays the record of what was transcribed. Versions live in `<lecture> · Versions/`:
//! `versions.json` (`{active, versions: [{id, name, created, model, covers, changed, removed}]}`)
//! and one `<id>.md` per version, in the transcript format.
//!
//! A version covers the original's first `covers` lines. What the app reads, shows, edits and
//! makes slides from is the *view*: the active version's lines, then the original's lines after
//! `covers` (recorded since the clean up), or the original alone when no version is active.
//! `slides::read_transcript` returns the view, so slides, notes and the transcript pane all use
//! the version the student picked. Deck line numbers are view indices: when the view changes
//! shape (another version, a clean up), `slides::remap_deck` moves them by time.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const SUFFIX: &str = " · Versions";
type Lines = Vec<(String, String)>;

/// "<lecture> · Versions/" beside the transcript.
pub fn dir(t: &Path) -> PathBuf {
    let stem = t.file_stem().unwrap_or_default().to_string_lossy();
    t.with_file_name(format!("{stem}{SUFFIX}"))
}

fn meta_path(t: &Path) -> PathBuf {
    dir(t).join("versions.json")
}

fn file(t: &Path, id: &str) -> PathBuf {
    dir(t).join(format!("{id}.md"))
}

/// `versions.json`, or an empty list.
pub fn meta(t: &Path) -> Value {
    std::fs::read_to_string(meta_path(t))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(|v| v["versions"].is_array())
        .unwrap_or_else(|| json!({ "active": null, "versions": [] }))
}

fn write_atomic(p: &Path, s: &str) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't save the version: {e}"))?;
    }
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, s).and_then(|_| std::fs::rename(&tmp, p)).map_err(|e| format!("Couldn't save the version: {e}"))
}

fn save_meta(t: &Path, m: &Value) -> Result<(), String> {
    write_atomic(&meta_path(t), &serde_json::to_string_pretty(m).unwrap())
}

fn entry<'a>(m: &'a Value, id: &str) -> Option<&'a Value> {
    m["versions"].as_array()?.iter().find(|v| v["id"] == id)
}

/// The active version's entry and file, if one is picked and still there.
fn active(t: &Path, m: &Value) -> Option<(Value, PathBuf)> {
    let id = m["active"].as_str()?;
    let e = entry(m, id)?.clone();
    let f = file(t, id);
    f.exists().then_some((e, f))
}

fn read_lines(p: &Path) -> Result<Lines, String> {
    Ok(crate::slides::parse_transcript(&std::fs::read_to_string(p).map_err(|e| format!("Can't read the version: {e}"))?).1)
}

/// The view (see the module docs) from the original's text. The engine passes the text it holds
/// for the lecture being recorded.
pub fn view_of(t: &Path, raw_text: &str) -> Result<(String, Lines), String> {
    let (title, raw) = crate::slides::parse_transcript(raw_text);
    let m = meta(t);
    let Some((e, f)) = active(t, &m) else { return Ok((title, raw)) };
    let covers = (e["covers"].as_u64().unwrap_or(0) as usize).min(raw.len());
    let mut lines = read_lines(&f)?;
    lines.extend(raw.into_iter().skip(covers));
    Ok((title, lines))
}

/// The view of the lecture `t`: what everything reads.
pub fn view(t: &Path) -> Result<(String, Lines), String> {
    view_of(t, &std::fs::read_to_string(t).map_err(|e| format!("Can't read the transcript: {e}"))?)
}

/// The versions as the page shows them: `{active, versions: [...], lines}` where `lines` is how
/// many view lines come from the active version (the rest are the original's, recorded since).
pub fn info(t: &Path) -> Value {
    let m = meta(t);
    let lines = active(t, &m).and_then(|(_, f)| read_lines(&f).ok()).map(|l| l.len());
    json!({ "active": if lines.is_some() { m["active"].clone() } else { Value::Null }, "versions": m["versions"], "lines": lines })
}

/// Where a hand edit at view line `i` goes.
#[derive(Debug, PartialEq)]
pub enum Target {
    /// The active version's line `i`.
    Version(usize),
    /// The original's line (the engine applies it: it may be recording).
    Original(usize),
}

pub fn route(t: &Path, op: &str, i: usize) -> Target {
    let m = meta(t);
    let Some((e, f)) = active(t, &m) else { return Target::Original(i) };
    let n = read_lines(&f).map(|l| l.len()).unwrap_or(0);
    let covers = e["covers"].as_u64().unwrap_or(0) as usize;
    // A line added right after the version's last line goes in the version.
    if i < n || (op == "insert" && i == n) { Target::Version(i) } else { Target::Original(covers + i - n) }
}

/// Held while a lecture's versions, view or deck change: hand edits run on the engine thread,
/// version switches and clean ups on others, and each reads the view the others write.
pub fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

/// Where the original's line `i` is in the view: the same line with no version active, after the
/// version's lines when it's past what the version covers, nowhere when the version replaces it.
pub fn view_index(t: &Path, i: usize) -> Option<usize> {
    let m = meta(t);
    let Some((e, f)) = active(t, &m) else { return Some(i) };
    let covers = e["covers"].as_u64().unwrap_or(0) as usize;
    let n = read_lines(&f).map(|l| l.len()).unwrap_or(0);
    (i >= covers).then(|| n + i - covers)
}

/// A hand edit to the active version's line `i` (see `slides::edit_body`); the deck follows.
pub fn edit(t: &Path, op: &str, i: usize, text: &str) -> Result<(), String> {
    let _l = lock();
    let m = meta(t);
    let (_, f) = active(t, &m).ok_or("That version isn't there any more.")?;
    let body = std::fs::read_to_string(&f).map_err(|e| format!("Can't read the version: {e}"))?;
    if let Some((body, shift)) = crate::slides::edit_body(&body, op, i, text)? {
        write_atomic(&f, &body)?;
        crate::slides::shift_deck(t, i, shift)?;
    }
    Ok(())
}

/// After a hand edit added (`shift` 1) or removed (-1) the original's line `i`: versions that
/// cover it keep covering the same lines.
pub fn original_edited(t: &Path, i: usize, shift: i64) {
    if shift == 0 || !meta_path(t).exists() {
        return;
    }
    let mut m = meta(t);
    let mut changed = false;
    for v in m["versions"].as_array_mut().into_iter().flatten() {
        let c = v["covers"].as_u64().unwrap_or(0) as i64;
        if (i as i64) < c {
            v["covers"] = json!((c + shift).max(0));
            changed = true;
        }
    }
    if changed {
        let _ = save_meta(t, &m);
    }
}

/// Makes `id` the active version (`None`: the original). The deck's line marks move by time.
pub fn set_active(t: &Path, id: Option<&str>) -> Result<(), String> {
    let _l = lock();
    activate(t, id)
}

fn activate(t: &Path, id: Option<&str>) -> Result<(), String> {
    let mut m = meta(t);
    if let Some(id) = id {
        if entry(&m, id).is_none() || !file(t, id).exists() {
            return Err("That version isn't there any more.".into());
        }
    }
    let (_, before) = view(t)?;
    m["active"] = json!(id);
    save_meta(t, &m)?;
    let (_, after) = view(t)?;
    crate::slides::remap_deck(t, &before, &after)
}

pub fn rename(t: &Path, id: &str, name: &str) -> Result<(), String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err("A version needs a name.".into());
    }
    let _l = lock();
    let mut m = meta(t);
    let v = m["versions"].as_array_mut().into_iter().flatten().find(|v| v["id"] == id).ok_or("That version isn't there any more.")?;
    v["name"] = json!(name.chars().take(60).collect::<String>());
    save_meta(t, &m)
}

/// Moves a version to the Trash. If it was the active one, the original is shown again.
pub fn delete(t: &Path, id: &str) -> Result<(), String> {
    let _l = lock();
    if entry(&meta(t), id).is_none() {
        return Err("That version isn't there any more.".into());
    }
    if meta(t)["active"] == id {
        activate(t, None)?;
    }
    #[cfg(not(test))]
    crate::docs::trash(&file(t, id)).map_err(|_| "Couldn't move the version to the Trash.".to_string())?;
    #[cfg(test)]
    let _ = std::fs::remove_file(file(t, id));
    let mut m = meta(t);
    if let Some(a) = m["versions"].as_array_mut() {
        a.retain(|v| v["id"] != id);
    }
    save_meta(t, &m)
}

/// Saves a clean up: the original's lines `from..to` as cleaned (`None`: removed), refused if
/// those lines changed since they were read. `from` 0 makes
/// a new version; otherwise it extends `extend`, which covers exactly `from` lines. The new or
/// extended version becomes the active one, and the deck follows. Returns its id.
pub fn save_clean(t: &Path, from: usize, to: usize, was: &[(String, String)], cleaned: &[(String, Option<String>)], extend: Option<&str>, model: &str) -> Result<String, String> {
    let _l = lock();
    let (title, before) = view(t)?;
    let mut m = meta(t);
    let changed = cleaned.iter().filter(|(_, c)| c.is_some()).count();
    let removed = cleaned.iter().filter(|(_, c)| c.is_none()).count();
    let raw = crate::slides::parse_transcript(&std::fs::read_to_string(t).map_err(|e| e.to_string())?).1;
    if to > raw.len() || raw[from..to] != *was || cleaned.len() != was.len() {
        return Err("The transcript changed while it was being cleaned up. Try again.".into());
    }
    let changed = changed - cleaned.iter().zip(raw.iter().skip(from)).filter(|((_, c), (_, o))| c.as_deref() == Some(o.as_str())).count();
    let block = |ts: &str, x: &str| format!("**[{ts}]** {x}\n\n");
    let new_lines: String = cleaned.iter().filter_map(|(ts, c)| c.as_ref().map(|x| block(ts, x))).collect();
    let id = match extend {
        Some(id) => {
            let v = m["versions"].as_array_mut().into_iter().flatten().find(|v| v["id"] == id).ok_or("That version isn't there any more.")?;
            if v["covers"].as_u64() != Some(from as u64) {
                return Err("The version changed while it was being cleaned up. Try again.".into());
            }
            let f = file(t, id);
            let mut body = std::fs::read_to_string(&f).map_err(|e| format!("Can't read the version: {e}"))?;
            // Drop the (empty) live-partial part, add the lines, end with an empty one again.
            while body.ends_with('\n') {
                body.pop();
            }
            body.push_str("\n\n");
            body.push_str(&new_lines);
            write_atomic(&f, &body)?;
            v["covers"] = json!(to);
            v["changed"] = json!(v["changed"].as_u64().unwrap_or(0) + changed as u64);
            v["removed"] = json!(v["removed"].as_u64().unwrap_or(0) + removed as u64);
            id.to_string()
        }
        None => {
            let ids: Vec<&str> = m["versions"].as_array().into_iter().flatten().filter_map(|v| v["id"].as_str()).collect();
            let id = (1..).map(|n| format!("clean-{n}")).find(|c| !ids.contains(&c.as_str()) && !file(t, c).exists()).unwrap();
            let now = chrono::Local::now();
            let head = format!("# {title}\n\n_Clean version · {} · Claude {model}_\n\n", now.format("%A %-d %B %Y, %H:%M"));
            write_atomic(&file(t, &id), &format!("{head}{new_lines}"))?;
            m["versions"].as_array_mut().unwrap().push(json!({
                "id": id, "name": "Clean", "created": now.format("%Y-%m-%dT%H:%M").to_string(), "model": model,
                "covers": to, "changed": changed, "removed": removed,
            }));
            id
        }
    };
    m["active"] = json!(id);
    save_meta(t, &m)?;
    let (_, after) = view(t)?;
    crate::slides::remap_deck(t, &before, &after)?;
    Ok(id)
}

/// The active version beside the original, line by line, for Compare: `[{t, old, new}]` with
/// `new` null for a line the clean up removed and `old` null for a line added by hand. Lines are
/// paired by time, in order. Lines recorded since the clean up come last, unchanged.
pub fn compare(t: &Path) -> Result<Value, String> {
    let raw = crate::slides::parse_transcript(&std::fs::read_to_string(t).map_err(|e| format!("Can't read the transcript: {e}"))?).1;
    let m = meta(t);
    let (e, f) = active(t, &m).ok_or("Pick a clean version to compare it with the original.")?;
    let covers = (e["covers"].as_u64().unwrap_or(0) as usize).min(raw.len());
    let mut rows = pair(&raw[..covers], &read_lines(&f)?);
    rows.extend(raw[covers..].iter().map(|(ts, x)| json!({ "t": secs(ts), "old": x, "new": x })));
    Ok(json!({ "type": "compare", "path": t.to_string_lossy(), "id": e["id"], "rows": rows }))
}

fn secs(t: &str) -> u64 {
    t.split(':').fold(0, |a, x| a * 60 + x.trim().parse::<u64>().unwrap_or(0))
}

fn pair(old: &[(String, String)], new: &[(String, String)]) -> Vec<Value> {
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < old.len() || j < new.len() {
        let (a, b) = (old.get(i).map(|l| secs(&l.0)), new.get(j).map(|l| secs(&l.0)));
        if a.is_some() && (b.is_none() || a < b) {
            out.push(json!({ "t": a, "old": old[i].1, "new": null }));
            i += 1;
        } else if a.is_some() && a == b {
            // Same time on both sides: the same line, unless the next original line is a better
            // match for it (the version dropped this one).
            let next_same = old.get(i + 1).is_some_and(|n| secs(&n.0) == a.unwrap() && n.1 == new[j].1 && old[i].1 != new[j].1);
            if next_same {
                out.push(json!({ "t": a, "old": old[i].1, "new": null }));
                i += 1;
                continue;
            }
            out.push(json!({ "t": a, "old": old[i].1, "new": new[j].1 }));
            i += 1;
            j += 1;
        } else {
            out.push(json!({ "t": b, "old": null, "new": new[j].1 }));
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lecture(name: &str, lines: &[(&str, &str)]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tl-versions-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let t = d.join("2026-11-19 10.00 Test.md");
        let body: String = lines.iter().map(|(ts, x)| format!("**[{ts}]** {x}\n\n")).collect();
        std::fs::write(&t, format!("# Test\n\n_Thursday_\n\n{body}")).unwrap();
        t
    }
    /// The original's lines, as a clean up reads them before asking Claude.
    fn snap(t: &Path) -> Lines {
        crate::slides::parse_transcript(&std::fs::read_to_string(t).unwrap()).1
    }
    fn texts(l: &Lines) -> Vec<&str> {
        l.iter().map(|x| x.1.as_str()).collect()
    }
    fn cleaned(v: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
        v.iter().map(|(t, x)| (t.to_string(), x.map(str::to_string))).collect()
    }

    /// A clean up becomes the view; lines recorded after it follow; the original is untouched,
    /// and picking the original again shows it.
    #[test]
    fn the_view_is_the_active_version_then_newer_lines() {
        let t = lecture("view", &[("00:00:01", "the radiant"), ("00:00:05", "can you know"), ("00:00:09", "eater is the rate")]);
        let raw = std::fs::read_to_string(&t).unwrap();
        let id = save_clean(&t, 0, 2, &snap(&t)[0..2], &cleaned(&[("00:00:01", Some("the gradient")), ("00:00:05", None)]), None, "sonnet").unwrap();
        assert_eq!(texts(&view(&t).unwrap().1), ["the gradient", "eater is the rate"]);
        assert_eq!(std::fs::read_to_string(&t).unwrap(), raw);
        assert_eq!(meta(&t)["versions"][0]["changed"], 1);
        assert_eq!(meta(&t)["versions"][0]["removed"], 1);
        // "Clean up the rest" extends it.
        save_clean(&t, 2, 3, &snap(&t)[2..3], &cleaned(&[("00:00:09", Some("eta is the rate"))]), Some(&id), "sonnet").unwrap();
        assert_eq!(texts(&view(&t).unwrap().1), ["the gradient", "eta is the rate"]);
        assert_eq!(info(&t)["lines"], 2);
        set_active(&t, None).unwrap();
        assert_eq!(texts(&view(&t).unwrap().1), ["the radiant", "can you know", "eater is the rate"]);
        // A second clean up is a new version.
        assert_eq!(save_clean(&t, 0, 3, &snap(&t)[0..3], &cleaned(&[("00:00:01", Some("a")), ("00:00:05", Some("b")), ("00:00:09", Some("c"))]), None, "opus").unwrap(), "clean-2");
    }

    /// Hand edits go to the version for its lines and to the original for the lines after it.
    #[test]
    fn edits_go_where_the_line_lives() {
        let t = lecture("route", &[("00:00:01", "a"), ("00:00:05", "b"), ("00:00:09", "c"), ("00:00:12", "d")]);
        assert_eq!(route(&t, "set", 1), Target::Original(1));
        save_clean(&t, 0, 3, &snap(&t)[0..3], &cleaned(&[("00:00:01", Some("A")), ("00:00:05", None), ("00:00:09", Some("C"))]), None, "sonnet").unwrap();
        // View: A, C, d.
        assert_eq!(route(&t, "set", 1), Target::Version(1));
        assert_eq!(route(&t, "insert", 2), Target::Version(2));
        assert_eq!(route(&t, "set", 2), Target::Original(3));
        edit(&t, "set", 1, "C!").unwrap();
        edit(&t, "insert", 2, "added").unwrap();
        assert_eq!(texts(&view(&t).unwrap().1), ["A", "C!", "added", "d"]);
        // With the original shown, removing a covered line keeps the version on the same lines.
        set_active(&t, None).unwrap();
        let body = std::fs::read_to_string(&t).unwrap();
        std::fs::write(&t, crate::slides::edit_body(&body, "delete", 0, "").unwrap().unwrap().0).unwrap();
        original_edited(&t, 0, -1);
        assert_eq!(meta(&t)["versions"][0]["covers"], 2);
        set_active(&t, Some("clean-1")).unwrap();
        assert_eq!(texts(&view(&t).unwrap().1), ["A", "C!", "added", "d"]);
    }

    /// Switching versions moves the deck's marks to the same moment of the lecture.
    #[test]
    fn the_deck_follows_the_view() {
        let t = lecture("deck", &[("00:00:01", "a"), ("00:00:05", "noise"), ("00:00:09", "c"), ("00:00:12", "d")]);
        let deck = crate::slides::deck_path(&t, ".json");
        std::fs::write(&deck, r#"{"batches":[{"from_line":0,"to_line":3,"slides":[]}]}"#).unwrap();
        save_clean(&t, 0, 4, &snap(&t)[0..4], &cleaned(&[("00:00:01", Some("A")), ("00:00:05", None), ("00:00:09", Some("C")), ("00:00:12", Some("D"))]), None, "sonnet").unwrap();
        let to = |p: &Path| serde_json::from_str::<Value>(&std::fs::read_to_string(p).unwrap()).unwrap()["batches"][0]["to_line"].clone();
        assert_eq!(to(&deck), 2); // "d" wasn't in the slides yet: it still isn't
        set_active(&t, None).unwrap();
        assert_eq!(to(&deck), 3);
    }

    /// A clean up whose lines changed while Claude worked saves nothing.
    #[test]
    fn a_clean_up_of_lines_that_changed_is_refused() {
        let t = lecture("changed", &[("00:00:01", "a"), ("00:00:05", "b"), ("00:00:09", "c")]);
        let was = snap(&t);
        crate::slides::edit_line(&t, None, "insert", 1, "mine").unwrap();
        let r = save_clean(&t, 0, 3, &was, &cleaned(&[("00:00:01", Some("A")), ("00:00:05", Some("B")), ("00:00:09", Some("C"))]), None, "sonnet");
        assert!(r.unwrap_err().contains("changed"));
        assert_eq!(texts(&view(&t).unwrap().1), ["a", "mine", "b", "c"]);
        assert!(info(&t)["versions"].as_array().unwrap().is_empty());
    }

    /// An edit to the original moves the deck by the line's place in the view as it is when the
    /// edit is applied; a line a version replaces isn't in the view, so the deck stays.
    #[test]
    fn original_edits_move_the_deck_by_the_view() {
        let t = lecture("shift", &[("00:00:01", "a"), ("00:00:05", "noise"), ("00:00:09", "c"), ("00:00:12", "d"), ("00:00:15", "e")]);
        save_clean(&t, 0, 3, &snap(&t)[0..3], &cleaned(&[("00:00:01", Some("A")), ("00:00:05", None), ("00:00:09", Some("C"))]), None, "sonnet").unwrap();
        // View: A, C, d, e. The slides cover A, C and d.
        let deck = crate::slides::deck_path(&t, ".json");
        std::fs::write(&deck, r#"{"batches":[{"from_line":0,"to_line":3,"slides":[]}]}"#).unwrap();
        let to = |p: &Path| serde_json::from_str::<Value>(&std::fs::read_to_string(p).unwrap()).unwrap()["batches"][0]["to_line"].clone();
        assert_eq!(view_index(&t, 3), Some(2));
        assert_eq!(view_index(&t, 1), None);
        crate::slides::edit_line(&t, None, "delete", 3, "").unwrap(); // the original's "d": view line 2
        assert_eq!(to(&deck), 2);
        assert_eq!(texts(&view(&t).unwrap().1), ["A", "C", "e"]);
        crate::slides::edit_line(&t, None, "delete", 1, "").unwrap(); // "noise", under the version
        assert_eq!(to(&deck), 2);
        assert_eq!(meta(&t)["versions"][0]["covers"], 2);
        assert_eq!(texts(&view(&t).unwrap().1), ["A", "C", "e"]);
    }

    /// A mark between two lines of the same second (a hand-added line) stays between them.
    #[test]
    fn a_mark_between_lines_of_the_same_second_stays() {
        let t = lecture("same", &[("00:00:01", "a"), ("00:00:01", "mine"), ("00:00:05", "b")]);
        let deck = crate::slides::deck_path(&t, ".json");
        std::fs::write(&deck, r#"{"batches":[{"from_line":0,"to_line":1,"slides":[]}]}"#).unwrap();
        save_clean(&t, 0, 3, &snap(&t)[0..3], &cleaned(&[("00:00:01", Some("A")), ("00:00:01", Some("Mine")), ("00:00:05", Some("B"))]), None, "sonnet").unwrap();
        set_active(&t, None).unwrap();
        let to = serde_json::from_str::<Value>(&std::fs::read_to_string(&deck).unwrap()).unwrap()["batches"][0]["to_line"].clone();
        assert_eq!(to, 1);
    }

    #[test]
    fn compare_pairs_lines_by_time() {
        let t = lecture("compare", &[("00:00:01", "a"), ("00:00:01", "noise"), ("00:00:05", "c"), ("00:00:09", "new")]);
        save_clean(&t, 0, 3, &snap(&t)[0..3], &cleaned(&[("00:00:01", Some("A")), ("00:00:01", None), ("00:00:05", Some("c"))]), None, "sonnet").unwrap();
        let rows = compare(&t).unwrap()["rows"].clone();
        assert_eq!(
            rows,
            json!([
                { "t": 1, "old": "a", "new": "A" },
                { "t": 1, "old": "noise", "new": null },
                { "t": 5, "old": "c", "new": "c" },
                { "t": 9, "old": "new", "new": "new" },
            ])
        );
        delete(&t, "clean-1").unwrap();
        assert_eq!(info(&t)["active"], Value::Null);
        assert_eq!(texts(&view(&t).unwrap().1).len(), 4);
    }
}
