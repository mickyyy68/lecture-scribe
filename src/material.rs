//! Course material: PDFs the student adds once per course. DeepSeek doesn't get them whole; it
//! sees a short index (file names and the first line of each page) and searches or reads pages
//! with tools, to check spelling, follow the professor's naming and point to pages.
//!
//! Text is extracted locally with macOS PDFKit and stored, page by page, in
//! ~/Library/Application Support/Lecture Scribe/material/<course>/<file>.json

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct Doc {
    pub name: String,
    pub pages: Vec<String>,
}

fn dir(course: &str) -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("Lecture Scribe").join("material").join(crate::glossary::slug(course))
}

/// Text of each page, via PDFKit.
pub fn pdf_pages(p: &Path) -> Result<Vec<String>, String> {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::{NSString, NSURL};
    #[link(name = "PDFKit", kind = "framework")]
    unsafe extern "C" {}
    objc2::rc::autoreleasepool(|_| unsafe {
        let cls = AnyClass::get(c"PDFDocument").ok_or("PDFKit non disponibile")?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy()));
        let doc: *mut AnyObject = msg_send![cls, alloc];
        let doc: *mut AnyObject = msg_send![doc, initWithURL: &*url];
        if doc.is_null() {
            return Err("Non riesco ad aprire il PDF.".to_string());
        }
        let n: usize = msg_send![doc, pageCount];
        let mut pages = Vec::with_capacity(n);
        for i in 0..n {
            let page: *mut AnyObject = msg_send![doc, pageAtIndex: i];
            let s: *mut NSString = if page.is_null() { std::ptr::null_mut() } else { msg_send![page, string] };
            pages.push(if s.is_null() { String::new() } else { (*s).to_string() });
        }
        let _: () = msg_send![doc, release];
        Ok(pages)
    })
}

/// Extracts a PDF and stores its text under the course. Returns the file's summary.
pub fn add(course: &str, pdf: &Path) -> Result<Value, String> {
    let name = pdf.file_name().unwrap_or_default().to_string_lossy().to_string();
    let pages: Vec<String> = pdf_pages(pdf)?.into_iter().map(|p| clean(&p)).collect();
    let chars: usize = pages.iter().map(|p| p.chars().count()).sum();
    if chars < 200 {
        return Err(format!("«{name}» non contiene testo (è una scansione?): serve l'OCR, non ancora supportato."));
    }
    let d = dir(course);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let v = json!({ "name": name, "source": pdf.to_string_lossy(), "added": chrono::Local::now().format("%Y-%m-%d").to_string(), "pages": pages });
    std::fs::write(d.join(format!("{}.json", crate::glossary::slug(&name))), v.to_string()).map_err(|e| e.to_string())?;
    Ok(summary(&v))
}

pub fn remove(course: &str, name: &str) -> Result<(), String> {
    std::fs::remove_file(dir(course).join(format!("{}.json", crate::glossary::slug(name)))).map_err(|e| e.to_string())
}

fn summary(v: &Value) -> Value {
    let pages = v["pages"].as_array().map_or(0, |a| a.len());
    let empty = v["pages"].as_array().into_iter().flatten().filter(|p| p.as_str().unwrap_or("").trim().is_empty()).count();
    json!({ "name": v["name"], "pages": pages, "emptyPages": empty, "added": v["added"] })
}

fn load_raw(course: &str) -> Vec<Value> {
    let mut v: Vec<Value> = std::fs::read_dir(dir(course))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok())
        .collect();
    v.sort_by(|a: &Value, b: &Value| a["name"].as_str().cmp(&b["name"].as_str()));
    v
}

pub fn list(course: &str) -> Vec<Value> {
    load_raw(course).iter().map(summary).collect()
}

pub fn load(course: &str) -> Vec<Doc> {
    load_raw(course)
        .into_iter()
        .map(|v| Doc {
            name: v["name"].as_str().unwrap_or("").to_string(),
            pages: v["pages"].as_array().into_iter().flatten().map(|p| p.as_str().unwrap_or("").to_string()).collect(),
        })
        .collect()
}

/// Collapses runs of spaces and blank lines, which PDF text is full of.
fn clean(s: &str) -> String {
    s.replace('\u{FFFC}', "").lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Lowercase, without accents, so "è" matches "e" and "Livello" matches "livello".
fn norm(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'á' | 'À' | 'Á' => 'a',
            'è' | 'é' | 'È' | 'É' => 'e',
            'ì' | 'í' | 'Ì' | 'Í' => 'i',
            'ò' | 'ó' | 'Ò' | 'Ó' => 'o',
            'ù' | 'ú' | 'Ù' | 'Ú' => 'u',
            c => c.to_lowercase().next().unwrap_or(c),
        })
        .collect()
}

/// What DeepSeek always sees: each file with its pages' first lines, so it knows where to look.
/// Stable while the material doesn't change, so it sits in the cached prefix.
pub fn index(docs: &[Doc]) -> String {
    let total: usize = docs.iter().map(|d| d.pages.len()).sum();
    let per_page = total <= 400;
    let mut out = String::new();
    for d in docs {
        out.push_str(&format!("«{}» ({} pagine)\n", d.name, d.pages.len()));
        if per_page {
            // Skip running headers and footers ("Unimore, Protocolli … 12"): lines that, without
            // their digits, appear on at least half the pages.
            let key = |l: &str| l.chars().filter(|c| !c.is_ascii_digit()).collect::<String>().trim().to_string();
            let mut freq: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for p in &d.pages {
                let mut ks: Vec<String> = p.lines().map(key).collect();
                ks.sort();
                ks.dedup();
                for k in ks {
                    *freq.entry(k).or_default() += 1;
                }
            }
            let boiler = |l: &str| d.pages.len() >= 4 && freq.get(&key(l)).copied().unwrap_or(0) * 2 >= d.pages.len();
            for (i, p) in d.pages.iter().enumerate() {
                let first: String = p.lines().find(|l| l.chars().any(|c| c.is_alphabetic()) && !boiler(l)).unwrap_or("").chars().take(70).collect();
                if !first.is_empty() {
                    out.push_str(&format!("  p.{} {}\n", i + 1, first));
                }
            }
        }
    }
    out
}

/// Pages that contain the query's words, best first, with a snippet around the match. Words not
/// found exactly (e.g. "dnsmask") also match near spellings in the material ("dnsmasq").
pub fn search(docs: &[Doc], query: &str, limit: usize) -> String {
    let terms: Vec<String> = norm(query).split(|c: char| !c.is_alphanumeric()).filter(|t| t.chars().count() >= 2).map(String::from).collect();
    if terms.is_empty() {
        return "Query vuota.".into();
    }
    let pages: Vec<(usize, usize, String)> =
        docs.iter().enumerate().flat_map(|(di, d)| d.pages.iter().enumerate().map(move |(pi, p)| (di, pi, norm(p)))).collect();
    // Each term, plus near spellings from the material when it isn't there as written.
    let alts: Vec<Vec<String>> = terms
        .iter()
        .map(|t| {
            let mut a = vec![t.clone()];
            if t.chars().count() >= 5 && !pages.iter().any(|(_, _, p)| p.contains(t.as_str())) {
                let mut near: Vec<String> = pages
                    .iter()
                    .flat_map(|(_, _, p)| p.split(|c: char| !c.is_alphanumeric()))
                    .filter(|w| (w.chars().count() as i64 - t.chars().count() as i64).abs() <= 2 && lev(w, t) <= 2)
                    .map(String::from)
                    .collect();
                near.sort();
                near.dedup();
                a.extend(near.into_iter().take(5));
            }
            a
        })
        .collect();
    let mut hits: Vec<(usize, usize, usize, usize, usize)> = Vec::new(); // (matched terms, count, doc, page, pos)
    for (di, pi, p) in &pages {
        let (mut matched, mut count, mut pos) = (0, 0, usize::MAX);
        for a in &alts {
            let found: Vec<usize> = a.iter().filter_map(|t| p.find(t.as_str())).collect();
            if !found.is_empty() {
                matched += 1;
                count += a.iter().map(|t| p.matches(t.as_str()).count()).sum::<usize>();
                pos = pos.min(*found.iter().min().unwrap());
            }
        }
        if matched > 0 {
            hits.push((matched, count, *di, *pi, pos));
        }
    }
    if hits.is_empty() {
        return format!("Nessun risultato per «{query}».");
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut out = String::new();
    for (m, _, di, pi, pos) in hits.iter().take(limit) {
        let d = &docs[*di];
        out.push_str(&format!("«{}» p.{} ({} di {} parole): …{}…\n\n", d.name, pi + 1, m, alts.len(), snippet(&d.pages[*pi], &pages_norm(&pages, *di, *pi), *pos)));
    }
    out
}

fn pages_norm(pages: &[(usize, usize, String)], di: usize, pi: usize) -> String {
    pages.iter().find(|(d, p, _)| *d == di && *p == pi).map(|x| x.2.clone()).unwrap_or_default()
}

/// ~300 characters of the original text around a byte position in its normalized form.
fn snippet(orig: &str, normed: &str, pos: usize) -> String {
    let at = normed.get(..pos).map_or(0, |s| s.chars().count());
    let chars: Vec<char> = orig.chars().collect();
    let (a, b) = (at.saturating_sub(120), (at + 200).min(chars.len()));
    chars[a.min(chars.len())..b].iter().collect::<String>().replace('\n', " / ")
}

pub fn read_page(docs: &[Doc], file: &str, page: usize) -> String {
    let f = norm(file);
    let Some(d) = docs.iter().find(|d| norm(&d.name) == f).or_else(|| docs.iter().find(|d| norm(&d.name).contains(&f))) else {
        return format!("File «{file}» non trovato.");
    };
    match d.pages.get(page.wrapping_sub(1)) {
        Some(p) => format!("«{}» p.{}:\n{}", d.name, page, p.chars().take(6000).collect::<String>()),
        None => format!("«{}» ha {} pagine.", d.name, d.pages.len()),
    }
}

/// True if the file exists and has that page; returns the file's exact name.
pub fn page_exists(docs: &[Doc], file: &str, page: usize) -> Option<String> {
    let f = norm(file);
    docs.iter().find(|d| norm(&d.name) == f).filter(|d| page >= 1 && page <= d.pages.len()).map(|d| d.name.clone())
}

/// Lowercased words of all the material, for the fact check.
pub fn words(docs: &[Doc]) -> std::collections::HashSet<String> {
    docs.iter().flat_map(|d| d.pages.iter()).flat_map(|p| norm(p).split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() >= 2).map(String::from).collect::<Vec<_>>()).collect()
}

/// Levenshtein distance (small strings).
pub fn lev(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Tool definitions sent to DeepSeek when the course has material.
pub fn tools() -> Value {
    json!([
        { "type": "function", "function": {
            "name": "search_material",
            "description": "Cerca parole nel materiale del corso (PDF). Restituisce le pagine migliori con un estratto. Utile per la grafia corretta di termini, comandi e nomi, e per trovare la pagina da citare.",
            "parameters": { "type": "object", "properties": {
                "query": { "type": "string", "description": "Parole da cercare, es. \"dnsmasq dhcp-range\"" }
            }, "required": ["query"] } } },
        { "type": "function", "function": {
            "name": "read_page",
            "description": "Legge il testo di una pagina del materiale del corso.",
            "parameters": { "type": "object", "properties": {
                "file": { "type": "string", "description": "Nome del file come nell'indice" },
                "page": { "type": "integer", "description": "Numero di pagina, da 1" }
            }, "required": ["file", "page"] } } }
    ])
}

/// Runs one tool call and returns its text result.
pub fn run_tool(docs: &[Doc], name: &str, args: &str) -> String {
    let a: Value = serde_json::from_str(args).unwrap_or(json!({}));
    match name {
        "search_material" => search(docs, a["query"].as_str().unwrap_or(""), 6),
        "read_page" => read_page(docs, a["file"].as_str().unwrap_or(""), a["page"].as_u64().unwrap_or(0) as usize),
        _ => format!("Strumento sconosciuto: {name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_near_spellings() {
        let docs = vec![Doc { name: "Lab3.pdf".into(), pages: vec!["Introduzione".into(), "Il server DHCP è dnsmasq\nfile /etc/dnsmasq.conf".into()] }];
        let r = search(&docs, "dnsmask", 3);
        assert!(r.contains("p.2"), "{r}");
        assert!(search(&docs, "configurazione inesistente", 3).starts_with("Nessun"));
        assert!(read_page(&docs, "lab3.pdf", 2).contains("dnsmasq.conf"));
        assert_eq!(page_exists(&docs, "Lab3.pdf", 3), None);
        assert_eq!(lev("dnsmask", "dnsmasq"), 1);
    }
}
