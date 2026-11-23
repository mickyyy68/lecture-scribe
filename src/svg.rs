//! SVG drawings: the slides' "svg" block and ```svg blocks in docs, written by Claude itself
//! (rules: `svg_prompt.txt`). The page draws them through `slides.js` `sanitizeSvg`, which is the
//! security boundary (decks, the app and docs all draw through it).
//!
//! A model can't see what it drew, so new drawings are checked once drawn: the page measures them
//! (`slides.js` `checkSvg`, asked with an `svgCheck` event and answered with the `svgChecked`
//! command), and those with problems go back to Claude once to fix (`repair`). A drawing still
//! broken after that (it doesn't draw, labels are cut off or on top of each other) is dropped:
//! a slide shows its "alt" sentence instead, a doc loses the block.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

const RULES: &str = include_str!("svg_prompt.txt");
/// The size rule, per place a drawing goes (`{size}` in svg_prompt.txt).
pub const SLIDE_SIZE: &str = "W is the width of the space it fills, in px: 540 in a column of layout \"two\", up to 1120 in layout \"one\"; H at most 400.";
pub const DOC_SIZE: &str = "W is 720; H at most 400.";
/// Widths of the space a drawing fills: a column of a two-column slide, a one-column slide, a doc.
pub const COLUMN: u32 = 540;
pub const FULL: u32 = 1120;
pub const DOC: u32 = 720;

/// The SVG DRAWINGS section of a prompt.
pub fn rules(size: &str) -> String {
    RULES.replace("{size}", size).trim_end().to_string()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub svg: String,
    /// The width of the space it fills, in slide px (so the page can tell if text gets too small).
    pub width: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub text: String,
    /// The drawing can't be shown like this: it doesn't draw, or labels are cut off or overlap.
    /// The others (literal colours, small text) are worth fixing but not worth losing it for.
    pub fatal: bool,
}

// ---- The page that measures drawings: the app's window, or a hidden one for the headless modes.

type Page = Box<dyn Fn(String) + Send>;
static PAGE: Mutex<Option<Page>> = Mutex::new(None);
static WAITING: Mutex<Vec<(u64, mpsc::Sender<Value>)>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(1);

/// `send` delivers an event to the page (as `window.onEngine(event)`).
pub fn connect(send: impl Fn(String) + Send + 'static) {
    *PAGE.lock().unwrap() = Some(Box::new(send));
}

/// The page's `svgChecked` answer.
pub fn answered(msg: &Value) {
    let id = msg["id"].as_u64().unwrap_or(0);
    let mut w = WAITING.lock().unwrap();
    if let Some(i) = w.iter().position(|(k, _)| *k == id) {
        let _ = w.remove(i).1.send(msg["problems"].clone());
    }
}

/// What the page found wrong with each drawing; None without a page, or if it didn't answer.
fn measured(items: &[Item]) -> Option<Vec<Vec<Problem>>> {
    let id = NEXT.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = mpsc::channel();
    WAITING.lock().unwrap().push((id, tx));
    let sent = PAGE.lock().unwrap().as_ref().map(|send| {
        let items: Vec<Value> = items.iter().map(|it| json!({ "svg": it.svg, "width": it.width })).collect();
        send(json!({ "type": "svgCheck", "id": id, "items": items }).to_string())
    });
    let answer = sent.and_then(|_| rx.recv_timeout(Duration::from_secs(20)).ok());
    WAITING.lock().unwrap().retain(|(k, _)| *k != id);
    let lists = answer?.as_array()?.clone();
    if lists.len() != items.len() {
        return None;
    }
    Some(
        lists
            .iter()
            .map(|l| {
                l.as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| Problem { text: p["text"].as_str().unwrap_or("").to_string(), fatal: p["fatal"].as_bool().unwrap_or(true) })
                    .filter(|p| !p.text.is_empty())
                    .collect()
            })
            .collect(),
    )
}

/// What can be told without drawing it.
fn basic(svg: &str) -> Vec<Problem> {
    let t = svg.trim();
    let fatal = |s: &str| vec![Problem { text: s.to_string(), fatal: true }];
    if t.is_empty() {
        fatal("The drawing is empty.")
    } else if !t.starts_with("<svg") || !t.ends_with("</svg>") {
        fatal("It must be exactly one <svg> element, from \"<svg\" to \"</svg>\", with nothing around it.")
    } else if t.len() > 100_000 {
        fatal(&format!("The SVG is too long ({} characters, at most 100000).", t.len()))
    } else {
        Vec::new()
    }
}

fn problems(items: &[Item]) -> Vec<Vec<Problem>> {
    let mut out: Vec<Vec<Problem>> = items.iter().map(|it| basic(&it.svg)).collect();
    let fine: Vec<usize> = (0..items.len()).filter(|&i| out[i].is_empty()).collect();
    if !fine.is_empty() {
        let drawn: Vec<Item> = fine.iter().map(|&i| items[i].clone()).collect();
        if let Some(m) = measured(&drawn) {
            for (i, p) in fine.into_iter().zip(m) {
                out[i] = p;
            }
        }
    }
    out
}

/// Asked to fix drawings: (index, drawing, its problems) → (index, new SVG) for those it fixed.
pub type Fixer<'a> = dyn FnMut(&[(usize, &Item, Vec<String>)]) -> Result<Vec<(usize, String)>, String> + 'a;

/// Checks the drawings and gives those with problems to `fix`, once. Each comes back as the
/// drawing to keep (the fixed one if it has nothing fatal left, else the original if it had
/// nothing fatal), or None if it can't be shown. Err only when the job was stopped.
pub fn repair(items: &[Item], fix: &mut Fixer) -> Result<Vec<Option<String>>, String> {
    let first = problems(items);
    let fatal = |p: &[Problem]| p.iter().any(|p| p.fatal);
    let mut out: Vec<Option<String>> = items.iter().zip(&first).map(|(it, p)| (!fatal(p)).then(|| it.svg.clone())).collect();
    let asks: Vec<(usize, &Item, Vec<String>)> =
        (0..items.len()).filter(|&i| !first[i].is_empty()).map(|i| (i, &items[i], first[i].iter().map(|p| p.text.clone()).collect())).collect();
    if asks.is_empty() {
        return Ok(out);
    }
    let fixed = match fix(&asks) {
        Ok(f) => f,
        Err(_) if crate::slides::stopped() => return Err(crate::slides::STOPPED.into()),
        Err(e) => {
            eprintln!("svg: the fix failed: {e}");
            Vec::new()
        }
    };
    let again: Vec<(usize, Item)> =
        fixed.into_iter().filter(|(i, _)| asks.iter().any(|a| a.0 == *i)).map(|(i, svg)| (i, Item { svg: svg.trim().to_string(), width: items[i].width })).collect();
    let second = problems(&again.iter().map(|(_, it)| it.clone()).collect::<Vec<_>>());
    for ((i, it), p) in again.into_iter().zip(second) {
        if !fatal(&p) {
            out[i] = Some(it.svg);
        }
    }
    Ok(out)
}

const FIX_PROMPT: &str = "You fix SVG drawings made for a university lecture's slides or notes. Each drawing comes with the problems found once it was drawn, and the width of the space it fills. Fix exactly those problems: move, re-anchor or shorten the placement of labels, enlarge the viewBox or the font, replace literal colours with the theme's variables. Keep every element, label, value and animation; add nothing, remove nothing, don't change the meaning.

{rules}

Answer ONLY with a JSON object {\"svgs\": [{\"i\": <the drawing's i>, \"svg\": \"<svg …>…</svg>\"}, …]}, one entry per drawing you were given, with the SVG's attributes in single quotes.";

/// Fixes drawings with one Claude Code call (no tools, no material).
pub fn fix_with_claude(asks: &[(usize, &Item, Vec<String>)], model: &str, effort: &str) -> Result<Vec<(usize, String)>, String> {
    let system = FIX_PROMPT.replace("{rules}", &rules("W is the width given with the drawing; H at most 400."));
    let list: Vec<Value> = asks.iter().map(|(i, it, p)| json!({ "i": i, "width": it.width, "problems": p, "svg": it.svg })).collect();
    let user = format!("DRAWINGS TO FIX:\n{}", serde_json::to_string_pretty(&list).unwrap());
    let run = crate::slides::run_claude(&system, &user, "", false, model, effort, false)?;
    let out = crate::slides::answer_json(&run.content)?;
    Ok(out["svgs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| Some((v["i"].as_u64()? as usize, v["svg"].as_str()?.to_string())))
        .collect())
}

/// Checks and fixes the "svg" blocks of new slides (`repair`). A drawing that can't be shown
/// becomes its "alt" sentence, or goes. Returns (fixed, dropped).
pub fn check_slides(slides: &mut [Value], fix: &mut Fixer) -> Result<(usize, usize), String> {
    let (mut at, mut items) = (Vec::new(), Vec::new());
    for (si, s) in slides.iter().enumerate() {
        let one = s["layout"] == "one" || s["right"].as_array().is_none_or(|r| r.is_empty());
        for col in ["left", "right"] {
            for (bi, b) in s[col].as_array().into_iter().flatten().enumerate() {
                if b["type"] == "svg" {
                    at.push((si, col, bi));
                    items.push(Item { svg: b["svg"].as_str().unwrap_or("").to_string(), width: if one { FULL } else { COLUMN } });
                }
            }
        }
    }
    if items.is_empty() {
        return Ok((0, 0));
    }
    let res = repair(&items, fix)?;
    let (mut fixed, mut dropped) = (0, 0);
    // Last first, so removing a block doesn't move the ones still to do.
    for ((si, col, bi), (r, it)) in at.into_iter().zip(res.into_iter().zip(&items)).rev() {
        match r {
            Some(svg) if svg == it.svg => {}
            Some(svg) => {
                fixed += 1;
                slides[si][col][bi]["svg"] = json!(svg);
            }
            None => {
                dropped += 1;
                let alt = slides[si][col][bi]["alt"].as_str().unwrap_or("").trim().to_string();
                if alt.is_empty() {
                    slides[si][col].as_array_mut().unwrap().remove(bi);
                } else {
                    slides[si][col][bi] = json!({ "type": "p", "text": alt });
                }
            }
        }
    }
    Ok((fixed, dropped))
}

/// Checks and fixes the ```svg blocks of a doc (`repair`); one that can't be shown is removed.
pub fn check_markdown(md: &str, fix: &mut Fixer) -> Result<String, String> {
    let lines: Vec<&str> = md.split('\n').collect();
    let mut blocks = Vec::new(); // (first line, closing line)
    let mut i = 0;
    while i < lines.len() {
        let open = lines[i].trim();
        if open.starts_with("```") || open.starts_with("~~~") {
            let fence = &open[..3];
            let end = (i + 1..lines.len()).find(|&j| lines[j].trim().starts_with(fence)).unwrap_or(lines.len());
            if open[3..].trim() == "svg" {
                blocks.push((i, end));
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    if blocks.is_empty() {
        return Ok(md.to_string());
    }
    let items: Vec<Item> = blocks.iter().map(|&(a, b)| Item { svg: lines[a + 1..b.min(lines.len())].join("\n"), width: DOC }).collect();
    let res = repair(&items, fix)?;
    let mut out: Vec<String> = Vec::new();
    let mut next = 0;
    for ((a, b), r) in blocks.into_iter().zip(res) {
        out.extend(lines[next..a].iter().map(|s| s.to_string()));
        if let Some(svg) = r {
            out.push(lines[a].to_string());
            out.push(svg);
            out.push(lines.get(b).map_or("```".to_string(), |s| s.to_string()));
        } else if out.last().is_some_and(|l| l.trim().is_empty()) && lines.get(b + 1).is_some_and(|l| l.trim().is_empty()) {
            out.pop(); // no double blank line where the block was
        }
        next = (b + 1).min(lines.len());
    }
    out.extend(lines[next..].iter().map(|s| s.to_string()));
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 540 200'><text x='20' y='40'>ok</text></svg>";
    const BAD: &str = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 540 200'><text x='520' y='40'>cut off</text></svg>";

    /// A fake page: a drawing is fatally wrong if it contains "cut off", cosmetically if "#333".
    fn fake_page() {
        connect(|ev| {
            let v: Value = serde_json::from_str(&ev).unwrap();
            let problems: Vec<Value> = v["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|it| {
                    let s = it["svg"].as_str().unwrap();
                    let mut p = Vec::new();
                    if s.contains("cut off") {
                        p.push(json!({ "text": "The label \"cut off\" goes outside the viewBox", "fatal": true }));
                    }
                    if s.contains("#333") {
                        p.push(json!({ "text": "It uses literal colours (#333)", "fatal": false }));
                    }
                    json!(p)
                })
                .collect();
            let msg = json!({ "cmd": "svgChecked", "id": v["id"], "problems": problems });
            std::thread::spawn(move || answered(&msg));
        });
    }

    // One test, since the page is global.
    #[test]
    fn repair_fixes_once_then_keeps_or_drops() {
        fake_page();
        let item = |s: &str, w| Item { svg: s.to_string(), width: w };

        // Nothing wrong: no fix call.
        let mut calls = 0;
        let r = repair(&[item(GOOD, COLUMN)], &mut |_: &[(usize, &Item, Vec<String>)]| {
            calls += 1;
            Ok(vec![])
        })
        .unwrap();
        assert_eq!((r, calls), (vec![Some(GOOD.to_string())], 0));

        // A fatal problem, fixed: the fixer sees only the faulty drawing, with its width and problem.
        let mut seen = Vec::new();
        let r = repair(&[item(GOOD, COLUMN), item(BAD, FULL)], &mut |a: &[(usize, &Item, Vec<String>)]| {
            seen = a.iter().map(|(i, it, p)| (*i, it.width, p.clone())).collect();
            Ok(vec![(1, BAD.replace("cut off", "fits"))])
        })
        .unwrap();
        assert_eq!(seen, vec![(1, FULL, vec!["The label \"cut off\" goes outside the viewBox".to_string()])]);
        assert_eq!(r, vec![Some(GOOD.to_string()), Some(BAD.replace("cut off", "fits"))]);

        // Still fatal after the fix, or the fix failed: dropped.
        assert_eq!(repair(&[item(BAD, COLUMN)], &mut |_: &[(usize, &Item, Vec<String>)]| Ok(vec![(0, BAD.to_string())])).unwrap(), vec![None]);
        assert_eq!(repair(&[item(BAD, COLUMN)], &mut |_: &[(usize, &Item, Vec<String>)]| Err("no".into())).unwrap(), vec![None]);
        // Not even an <svg>: no page needed to know.
        assert_eq!(repair(&[item("<div>hi</div>", COLUMN)], &mut |_: &[(usize, &Item, Vec<String>)]| Ok(vec![])).unwrap(), vec![None]);

        // Only cosmetic: kept as it was if the fix made it worse.
        let grey = GOOD.replace("<text", "<text style='fill:#333'");
        let r = repair(&[item(&grey, COLUMN)], &mut |_: &[(usize, &Item, Vec<String>)]| Ok(vec![(0, BAD.to_string())])).unwrap();
        assert_eq!(r, vec![Some(grey.clone())]);

        // Slides: a broken drawing becomes its alt sentence, or goes when it has none; widths follow the layout.
        let mut slides = vec![
            json!({ "layout": "two", "left": [{ "type": "list", "items": ["a"] }], "right": [{ "type": "svg", "alt": "A box.", "svg": BAD }] }),
            json!({ "layout": "one", "left": [{ "type": "svg", "svg": BAD }, { "type": "svg", "svg": GOOD }] }),
        ];
        let mut widths = Vec::new();
        let n = check_slides(&mut slides, &mut |a: &[(usize, &Item, Vec<String>)]| {
            widths = a.iter().map(|(_, it, _)| it.width).collect();
            Ok(vec![])
        })
        .unwrap();
        assert_eq!(n, (0, 2));
        assert_eq!(widths, vec![COLUMN, FULL]);
        assert_eq!(slides[0]["right"], json!([{ "type": "p", "text": "A box." }]));
        assert_eq!(slides[1]["left"], json!([{ "type": "svg", "svg": GOOD }]));

        // Docs: the fixed drawing replaces the block's content; a broken one goes with its blank line.
        let md = format!("# Notes\n\nText.\n\n```svg\n{BAD}\n```\n\nMore.\n\n```svg\n{}\n```\n\nEnd.\n", BAD.replace("540", "541"));
        let out = check_markdown(&md, &mut |a: &[(usize, &Item, Vec<String>)]| {
            assert_eq!(a.iter().map(|x| x.1.width).collect::<Vec<_>>(), vec![DOC, DOC]);
            Ok(vec![(0, GOOD.to_string())])
        })
        .unwrap();
        assert_eq!(out, format!("# Notes\n\nText.\n\n```svg\n{GOOD}\n```\n\nMore.\n\nEnd.\n"));
        // No drawings: unchanged, byte for byte.
        let plain = "# A\n\n```chart\n{}\n```\n";
        assert_eq!(check_markdown(plain, &mut |_: &[(usize, &Item, Vec<String>)]| unreachable!()).unwrap(), plain);
    }
}
