use transcribe_lecture::engine::{self, Cmd, Event, Source, StartOpts};
use transcribe_lecture::paths::{self, out_dir};
use transcribe_lecture::{audio, docs, glossary, material, models, slides, theme};
use serde_json::{json, Value};
use std::path::PathBuf;
use tao::{
    dpi::LogicalSize,
    event::{Event as TaoEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

/// The window's page: ui.html with its design tokens (app.css), the slide theme and renderer inlined. The chosen custom theme
/// is applied by the page itself (`init` carries its CSS), so it can change without a reload.
fn ui() -> String {
    include_str!("ui.html")
        .replace("/*BUNDLE_CSS*/", theme::BASE_CSS)
        .replace("/*TOKENS_CSS*/", theme::DEFAULT_CSS)
        .replace("/*SLIDES_CSS*/", slides::SLIDES_CSS)
        .replace("/*APP_CSS*/", include_str!("app.css"))
        .replace("/*SLIDES_JS*/", slides::SLIDES_JS)
        .replace("/*DOCS_JS*/", include_str!("docs.js"))
        .replace("/*KATEX_CSS*/", slides::KATEX_CSS)
        .replace("/*KATEX_JS*/", slides::KATEX_JS)
        .replace("/*CLAUDE_SVG*/", include_str!("claude-symbol.svg").trim())
}

static SLIDES_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static DRAFT_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CHECK_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static DOWNLOADING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn init_payload() -> Value {
    let settings = paths::load_settings();
    json!({
        "type": "init",
        "models": models::discover(),
        "modelBytes": models::DOWNLOAD_BYTES,
        "devices": audio::input_device_names(),
        "defaultDevice": audio::default_device_name(),
        "themeCss": theme::css(settings["theme"].as_str().unwrap_or("")),
        "themes": theme::list(),
        "settings": settings,
        "outDir": out_dir().to_string_lossy(),
        "recent": slides::recent(&out_dir()),
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Claude Code starts this to search the course PDFs (see slides::run_claude). Nothing else
    // may write to stdout here, so it runs before the transcription engine is set up.
    // For the UI tests (tests/ui): the exact page the window shows.
    if args.iter().any(|a| a == "--dump-ui") {
        return print!("{}", ui());
    }
    if args.iter().any(|a| a == "--mcp-material") {
        let course = args.iter().position(|a| a == "--course").and_then(|j| args.get(j + 1)).cloned().unwrap_or_default();
        return material::serve_mcp(&course);
    }
    if std::env::var_os("TRANSCRIBE_LECTURE_DEBUG").is_none() {
        transcribe_cpp::disable_logging();
    }
    transcribe_cpp::init_backends_default().expect("init ggml backends");
    // Headless: `transcribe-lecture --add-pdf file.pdf [--course "Networks"]`
    if let Some(i) = args.iter().position(|a| a == "--add-pdf") {
        let course = args.iter().position(|a| a == "--course").map(|j| args[j + 1].clone()).unwrap_or_default();
        match material::add(&course, std::path::Path::new(&args[i + 1])) {
            Ok(v) => println!("added {v}"),
            Err(e) => eprintln!("error: {e}"),
        }
        return;
    }
    // Headless: `transcribe-lecture --check file.md [--course X] [--model sonnet] [--effort low]`.
    // A real Claude Code call; prints the proposed fixes and corrections, writes nothing.
    if let Some(i) = args.iter().position(|a| a == "--check") {
        let opt = |k: &str| args.iter().position(|a| a == k).map(|j| args[j + 1].clone());
        let (course, model, effort) = (opt("--course").unwrap_or_default(), opt("--model").unwrap_or_else(|| slides::DEFAULT_MODEL.into()), opt("--effort").unwrap_or_else(|| slides::DEFAULT_EFFORT.into()));
        match transcribe_lecture::check::check(std::path::Path::new(&args[i + 1]), &course, &model, &effort) {
            Ok(v) => {
                for f in v["fixes"].as_array().into_iter().flatten() {
                    println!("#{} [{}] {}\n   - {}\n   + {}", f["i"], f["t"], f["why"].as_str().unwrap_or(""), f["old"].as_str().unwrap_or(""), f["text"].as_str().unwrap_or(""));
                }
                println!("\ncorrections: {}\nterms: {}\nusage: {}\nlookups: {}", v["corrections"], v["terms"], v["usage"], v["lookups"]);
            }
            Err(e) => eprintln!("error: {e}"),
        }
        return;
    }
    // Headless: `transcribe-lecture --draft file.md [--course X] [--model sonnet] [--effort high] [--lang it] [--hint "…"]`.
    // A real Claude Code call; writes the notes into "<lecture> · Docs/", so run it on a copy.
    if let Some(i) = args.iter().position(|a| a == "--draft") {
        let opt = |k: &str| args.iter().position(|a| a == k).map(|j| args[j + 1].clone()).unwrap_or_default();
        match docs::draft(std::path::Path::new(&args[i + 1]), &opt("--course"), &opt("--model"), &opt("--effort"), &opt("--lang"), &opt("--hint")) {
            Ok(v) => println!("saved {}\nusage: {}\nlookups: {}", v["doc"].as_str().unwrap_or(""), v["usage"], v["lookups"]),
            Err(e) => eprintln!("error: {e}"),
        }
        return;
    }
    // Headless: `transcribe-lecture --slides transcript.md [--course "Networks"] [--model opus] [--effort high] [--lang it] [--hint "…"]`
    if let Some(i) = args.iter().position(|a| a == "--slides") {
        let course = args.iter().position(|a| a == "--course").map(|j| args[j + 1].clone()).unwrap_or_default();
        let model = args.iter().position(|a| a == "--model").map(|j| args[j + 1].clone()).unwrap_or_else(|| slides::DEFAULT_MODEL.into());
        let effort = args.iter().position(|a| a == "--effort").map(|j| args[j + 1].clone()).unwrap_or_else(|| slides::DEFAULT_EFFORT.into());
        let lang = args.iter().position(|a| a == "--lang").map(|j| args[j + 1].clone()).unwrap_or_default();
        let mode = if args.iter().any(|a| a == "--rebuild") {
            slides::Mode::Rebuild
        } else if args.iter().any(|a| a == "--review") {
            slides::Mode::Review
        } else {
            slides::Mode::Next
        };
        let hint = args.iter().position(|a| a == "--hint").map(|j| args[j + 1].clone()).unwrap_or_default();
        match slides::generate(std::path::Path::new(&args[i + 1]), &course, mode, &model, &effort, &lang, &hint) {
            Ok(v) => println!(
                "{} slides (+{}, replaced last: {}, flagged: {}) · usage {}",
                v["slides"].as_array().map_or(0, |a| a.len()),
                v["added"],
                v["replaced"],
                v["flagged"],
                v["usage"]
            ),
            Err(e) => eprintln!("error: {e}"),
        }
        return;
    }
    // Headless: `transcribe-lecture --make-theme "Name" "description" [--model opus]`
    if let Some(i) = args.iter().position(|a| a == "--make-theme") {
        let (name, description) = (args.get(i + 1).cloned().unwrap_or_default(), args.get(i + 2).cloned().unwrap_or_default());
        let model = args.iter().position(|a| a == "--model").map(|j| args[j + 1].clone()).unwrap_or_else(|| slides::DEFAULT_MODEL.into());
        match theme::create(&name, &description, &model) {
            Ok(v) => println!("saved {}", theme::dir().join(format!("{}.css", v["name"].as_str().unwrap_or(""))).display()),
            Err(e) => eprintln!("error: {e}"),
        }
        return;
    }
    // Headless: `transcribe-lecture --download-model`
    if args.iter().any(|a| a == "--download-model") {
        match models::download(&|n| eprint!("\r{:.0} / {:.0} MB", n as f64 / 1e6, models::DOWNLOAD_BYTES as f64 / 1e6)) {
            Ok(p) => println!("\nsaved {}", p.display()),
            Err(e) => eprintln!("\nerror: {e}"),
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--simulate") {
        return simulate(&args, i);
    }

    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let emit_proxy = proxy.clone();
    let mut engine = engine::spawn(Box::new(move |e: Event| {
        // "recording", "paused" or "idle", for scripts/install.sh: never replace the app mid-lecture.
        if let Event::State { state, .. } = &e {
            let _ = std::fs::write(paths::data_dir().join("state"), state);
        }
        if let Ok(s) = serde_json::to_string(&e) {
            let _ = emit_proxy.send_event(s);
        }
    }));
    let tx = engine.tx.clone();
    let _ = std::fs::write(paths::data_dir().join("state"), "idle"); // none left from a crash

    let window = WindowBuilder::new()
        .with_title(paths::APP_NAME)
        .with_inner_size(LogicalSize::new(980.0, 760.0))
        .with_min_inner_size(LogicalSize::new(560.0, 420.0))
        .build(&event_loop)
        .expect("window");

    let ipc_proxy = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_html(ui())
        .with_devtools(cfg!(debug_assertions))
        .with_ipc_handler(move |req| {
            let Ok(msg) = serde_json::from_str::<Value>(req.body()) else { return };
            let s = |k: &str| msg.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
            match msg["cmd"].as_str().unwrap_or("") {
                "ready" | "refresh" => {
                    let _ = ipc_proxy.send_event(init_payload().to_string());
                }
                "preload" => {
                    let _ = tx.send(Cmd::Preload(s("model")));
                }
                "start" => {
                    let device = Some(s("device")).filter(|d| !d.is_empty());
                    let language = Some(s("language")).filter(|d| !d.is_empty() && d != "auto");
                    let _ = tx.send(Cmd::Start(StartOpts {
                        source: Source::Mic(device),
                        model: s("model"),
                        model_label: s("modelLabel"),
                        language,
                        title: s("title"),
                        save_audio: msg["saveAudio"].as_bool().unwrap_or(true),
                        out_dir: out_dir(),
                        course: s("course"),
                        continue_path: Some(s("continue")).filter(|p| !p.is_empty()).map(PathBuf::from),
                    }));
                }
                "pause" => {
                    let _ = tx.send(Cmd::Pause);
                }
                "resume" => {
                    let _ = tx.send(Cmd::Resume);
                }
                "stop" => {
                    let _ = tx.send(Cmd::Stop);
                }
                "open" => {
                    let _ = std::process::Command::new("open").arg(s("path")).spawn();
                }
                "reveal" => {
                    let _ = std::process::Command::new("open").args(["-R", &s("path")]).spawn();
                }
                "openFolder" => {
                    let d = out_dir();
                    let _ = std::fs::create_dir_all(&d);
                    let _ = std::process::Command::new("open").arg(d).spawn();
                }
                // Slides: Claude Code runs on its own thread; the result comes back as an event.
                "slides" => {
                    use std::sync::atomic::Ordering;
                    if SLIDES_BUSY.swap(true, Ordering::SeqCst) {
                        return;
                    }
                    let (path, course, proxy) = (PathBuf::from(s("path")), s("course"), ipc_proxy.clone());
                    let mode = if msg["rebuild"].as_bool() == Some(true) { slides::Mode::Rebuild } else { slides::Mode::parse(&s("mode")) };
                    let (model, effort) = (s("model"), s("effort"));
                    let (lang, hint) = (s("lang"), s("hint"));
                    std::thread::spawn(move || {
                        let r = slides::job("slides", || slides::generate(&path, &course, mode, &model, &effort, &lang, &hint));
                        SLIDES_BUSY.store(false, Ordering::SeqCst);
                        let ev = r.unwrap_or_else(|e| json!({ "type": "slidesError", "message": e, "path": path.to_string_lossy() }));
                        let _ = proxy.send_event(ev.to_string());
                    });
                }
                // Stop: kills the Claude Code run of a job; its thread then reports "Stopped.".
                "cancel" => {
                    slides::cancel(&s("kind"));
                }
                "slidesState" | "slidesUndo" => {
                    let path = PathBuf::from(s("path"));
                    let r = if msg["cmd"] == "slidesUndo" { slides::undo(&path) } else { slides::state(&path) };
                    let ev = r.unwrap_or_else(|e| json!({ "type": "slidesError", "message": e, "path": s("path") }));
                    let _ = ipc_proxy.send_event(ev.to_string());
                }
                // Check transcript: Claude Code proofreads the lecture; the fixes come back for review.
                "checkTranscript" => {
                    use std::sync::atomic::Ordering;
                    if CHECK_BUSY.swap(true, Ordering::SeqCst) {
                        return;
                    }
                    let (path, course, model, effort, proxy) = (PathBuf::from(s("path")), s("course"), s("model"), s("effort"), ipc_proxy.clone());
                    std::thread::spawn(move || {
                        let r = slides::job("check", || transcribe_lecture::check::check(&path, &course, &model, &effort));
                        CHECK_BUSY.store(false, Ordering::SeqCst);
                        let ev = r.unwrap_or_else(|e| json!({ "type": "checkError", "message": e, "path": path.to_string_lossy() }));
                        let _ = proxy.send_event(ev.to_string());
                    });
                }
                // Accepted from a check: corrections and terms into the course glossary.
                "glossaryAdd" => {
                    let course = s("course");
                    glossary::merge(&course, &json!({ "corrections": msg["corrections"], "terms": msg["terms"] }));
                    let g = glossary::load(&course);
                    let ev = json!({ "type": "glossary", "course": course, "corrections": g["corrections"], "terms": g["terms"] });
                    let _ = ipc_proxy.send_event(ev.to_string());
                }
                "glossary" | "glossaryRemove" => {
                    let course = s("course");
                    let g = if msg["cmd"] == "glossaryRemove" {
                        glossary::remove(&course, &s("kind"), &s("value")).unwrap_or_else(|_| glossary::load(&course))
                    } else {
                        glossary::load(&course)
                    };
                    let ev = json!({ "type": "glossary", "course": course, "corrections": g["corrections"], "terms": g["terms"] });
                    let _ = ipc_proxy.send_event(ev.to_string());
                }
                // Course material: the file picker and PDF extraction run off the UI thread.
                "material" | "materialRemove" | "materialAdd" => {
                    let (cmd, course, name, proxy) = (s("cmd"), s("course"), s("name"), ipc_proxy.clone());
                    std::thread::spawn(move || {
                        let mut ev = json!({ "type": "material", "course": course });
                        if cmd == "materialRemove" {
                            let _ = material::remove(&course, &name);
                        }
                        if cmd == "materialAdd" {
                            let (mut added, mut errors) = (Vec::new(), Vec::new());
                            for p in pick_pdfs() {
                                match material::add(&course, &p) {
                                    Ok(v) => added.push(v["name"].clone()),
                                    Err(e) => errors.push(json!(e)),
                                }
                            }
                            ev["added"] = json!(added);
                            ev["errors"] = json!(errors);
                        }
                        ev["files"] = json!(material::list(&course));
                        let _ = proxy.send_event(ev.to_string());
                    });
                }
                // Docs: files beside the lecture (see docs.rs). Drafting runs on its own thread.
                "docsList" | "docRead" | "docSave" | "docCreate" | "docRename" | "docDelete" | "docImage" => {
                    let _ = ipc_proxy.send_event(docs::handle(&msg).to_string());
                }
                // The file picker would block the window: its own thread.
                "docPickImage" => {
                    let (msg, proxy) = (msg.clone(), ipc_proxy.clone());
                    std::thread::spawn(move || {
                        let _ = proxy.send_event(docs::handle(&msg).to_string());
                    });
                }
                "docDraft" => {
                    use std::sync::atomic::Ordering;
                    if DRAFT_BUSY.swap(true, Ordering::SeqCst) {
                        return;
                    }
                    let (path, course, model, effort, lang, hint, proxy) = (PathBuf::from(s("path")), s("course"), s("model"), s("effort"), s("lang"), s("hint"), ipc_proxy.clone());
                    std::thread::spawn(move || {
                        let r = slides::job("draft", || docs::draft(&path, &course, &model, &effort, &lang, &hint));
                        DRAFT_BUSY.store(false, Ordering::SeqCst);
                        let ev = r.unwrap_or_else(|e| json!({ "type": "docDraftError", "message": e, "path": path.to_string_lossy() }));
                        let _ = proxy.send_event(ev.to_string());
                    });
                }
                "lineEdit" => {
                    let i = msg["i"].as_u64().unwrap_or(u64::MAX) as usize;
                    let _ = tx.send(Cmd::EditLine { path: PathBuf::from(s("path")), op: s("op"), i, text: s("text") });
                }
                // The lecture picker asks again each time it opens, so new lectures show up.
                "recentList" => {
                    let _ = ipc_proxy.send_event(json!({ "type": "recent", "recent": slides::recent(&out_dir()) }).to_string());
                }
                // Rename: not while Claude is writing the lecture's slides, check or notes (they'd
                // be written under the old name); the engine refuses while recording.
                "renameLecture" => {
                    use std::sync::atomic::Ordering;
                    let from = s("path");
                    let busy = [&SLIDES_BUSY, &CHECK_BUSY, &DRAFT_BUSY].iter().any(|b| b.load(Ordering::SeqCst));
                    let reply = move |r: Result<PathBuf, String>, proxy: &tao::event_loop::EventLoopProxy<String>| {
                        let ev = match r {
                            Ok(p) => slides::transcript_payload(&p).map(|mut v| {
                                v["type"] = json!("renamed");
                                v["from"] = json!(from);
                                v["recent"] = json!(slides::recent(&out_dir()));
                                v
                            }),
                            Err(e) => Err(e),
                        };
                        let ev = ev.unwrap_or_else(|e| json!({ "type": "renamed", "from": from, "error": e }));
                        let _ = proxy.send_event(ev.to_string());
                    };
                    if busy {
                        reply(Err("Wait for Claude to finish before renaming the lecture.".into()), &ipc_proxy);
                    } else {
                        let proxy = ipc_proxy.clone();
                        let _ = tx.send(Cmd::Rename { path: PathBuf::from(s("path")), title: s("title"), done: Box::new(move |r| reply(r, &proxy)) });
                    }
                }
                "loadTranscript" => {
                    let ev = slides::transcript_payload(&PathBuf::from(s("path")))
                        .unwrap_or_else(|e| json!({ "type": "error", "message": e }));
                    let _ = ipc_proxy.send_event(ev.to_string());
                }
                // Theme chosen: its CSS for the page, and the open deck's HTML is written again.
                "theme" => {
                    let path = s("path");
                    if !path.is_empty() {
                        let _ = slides::export(std::path::Path::new(&path));
                    }
                    let ev = json!({ "type": "theme", "name": s("name"), "css": theme::css(&s("name")), "themes": theme::list() });
                    let _ = ipc_proxy.send_event(ev.to_string());
                }
                "themeCreate" => {
                    let (name, description, proxy) = (s("name"), s("description"), ipc_proxy.clone());
                    let model = Some(s("model")).filter(|m| slides::MODELS.contains(&m.as_str())).unwrap_or_else(|| slides::DEFAULT_MODEL.into());
                    std::thread::spawn(move || {
                        let ev = match theme::create(&name, &description, &model) {
                            Ok(mut v) => {
                                v["type"] = json!("themeCreated");
                                v
                            }
                            Err(e) => json!({ "type": "themeError", "message": e }),
                        };
                        let _ = proxy.send_event(ev.to_string());
                    });
                }
                "themeDelete" => {
                    let ev = match theme::delete(&s("name")) {
                        Ok(()) => json!({ "type": "theme", "name": "", "css": "", "themes": theme::list() }),
                        Err(e) => json!({ "type": "themeError", "message": e }),
                    };
                    let _ = ipc_proxy.send_event(ev.to_string());
                }
                "openThemes" => {
                    let d = theme::dir();
                    let _ = std::fs::create_dir_all(&d);
                    let _ = std::process::Command::new("open").arg(d).spawn();
                }
                // Setup: download the speech model, test the microphone, look for Claude Code.
                "downloadModel" => {
                    use std::sync::atomic::Ordering;
                    if DOWNLOADING.swap(true, Ordering::SeqCst) {
                        return;
                    }
                    let proxy = ipc_proxy.clone();
                    std::thread::spawn(move || {
                        let p2 = proxy.clone();
                        let r = models::download(&move |n| {
                            let _ = p2.send_event(json!({ "type": "modelProgress", "done": n, "total": models::DOWNLOAD_BYTES }).to_string());
                        });
                        DOWNLOADING.store(false, Ordering::SeqCst);
                        let ev = match r {
                            Ok(_) => json!({ "type": "modelDownloaded" }),
                            Err(e) => json!({ "type": "modelDownloadError", "message": e }),
                        };
                        let _ = proxy.send_event(ev.to_string());
                        let _ = proxy.send_event(init_payload().to_string());
                    });
                }
                "micTest" => {
                    let (device, proxy) = (s("device"), ipc_proxy.clone());
                    std::thread::spawn(move || {
                        let ev = match audio::test_level(Some(&device).filter(|d| !d.is_empty()).map(|d| d.as_str()), 3.0) {
                            Ok(db) => json!({ "type": "micTest", "db": db }),
                            Err(e) => json!({ "type": "micTest", "error": e }),
                        };
                        let _ = proxy.send_event(ev.to_string());
                    });
                }
                "checkClaude" => {
                    let proxy = ipc_proxy.clone();
                    std::thread::spawn(move || {
                        let version = slides::claude_bin().and_then(|b| std::process::Command::new(b).arg("--version").output().ok()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
                        let _ = proxy.send_event(json!({ "type": "claudeCheck", "found": version.is_some(), "version": version }).to_string());
                    });
                }
                // Slide style page: three sample slides in the chosen style (and, with feedback, the
                // student's own rules rewritten to include it).
                "styleSample" => {
                    let proxy = ipc_proxy.clone();
                    let (course, model, effort, lang, feedback) = (s("course"), s("model"), s("effort"), s("lang"), s("feedback"));
                    let (style, previous, builtin) = (msg["style"].clone(), msg["previous"].clone(), msg["builtin"].as_bool() == Some(true));
                    std::thread::spawn(move || {
                        let r = slides::sample(&out_dir(), builtin, &course, &model, &effort, &lang, &style, &feedback, &previous);
                        let ev = r.unwrap_or_else(|e| json!({ "type": "styleError", "message": e }));
                        let _ = proxy.send_event(ev.to_string());
                    });
                }
                "saveSettings" => {
                    let p = paths::settings_path();
                    let _ = std::fs::create_dir_all(p.parent().unwrap());
                    let _ = std::fs::write(p, msg["settings"].to_string());
                }
                _ => {}
            }
        })
        .build(&window)
        .expect("webview");

    let menu = app_menu();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            // NSApp exists from here on: the menu bar gives text fields Cmd-C/V/X/A/Z.
            TaoEvent::NewEvents(tao::event::StartCause::Init) => menu.init_for_nsapp(),
            TaoEvent::UserEvent(js) => {
                let _ = webview.evaluate_script(&format!("window.onEngine({js})"));
            }
            TaoEvent::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                engine.shutdown(); // finishes and saves any running recording
                *control_flow = ControlFlow::Exit;
            }
            // Quit from the menu or Dock (Cmd-Q) skips CloseRequested: save the recording here too.
            TaoEvent::LoopDestroyed => engine.shutdown(),
            _ => {}
        }
    });
}

/// The menu bar. Without an Edit menu, macOS doesn't send Cut, Copy, Paste, Select All and
/// Undo to the page, so their shortcuts would do nothing in text fields and transcript lines.
fn app_menu() -> muda::Menu {
    use muda::{Menu, PredefinedMenuItem as P, Submenu};
    let menu = Menu::new();
    let app = Submenu::with_items(paths::APP_NAME, true, &[
        &P::about(None, None), &P::separator(), &P::services(None), &P::separator(),
        &P::hide(None), &P::hide_others(None), &P::show_all(None), &P::separator(), &P::quit(None),
    ]);
    let edit = Submenu::with_items("Edit", true, &[
        &P::undo(None), &P::redo(None), &P::separator(),
        &P::cut(None), &P::copy(None), &P::paste(None), &P::select_all(None),
    ]);
    let window = Submenu::with_items("Window", true, &[&P::minimize(None), &P::fullscreen(None), &P::separator(), &P::close_window(None)]);
    for m in [app, edit, window].into_iter().flatten() {
        let _ = menu.append(&m);
    }
    menu
}

/// The macOS file picker, through AppleScript (no extra dependencies). Empty if cancelled.
fn pick_pdfs() -> Vec<PathBuf> {
    let script = r#"set fs to choose file with prompt "Course material (PDF)" of type {"com.adobe.pdf"} with multiple selections allowed
set out to ""
repeat with f in fs
set out to out & POSIX path of f & linefeed
end repeat
return out"#;
    std::process::Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.trim().is_empty()).map(PathBuf::from).collect())
        .unwrap_or_default()
}

/// Headless test: `transcribe-lecture --simulate file.wav [--model path] [--speed N] [--lang it]`
fn simulate(args: &[String], i: usize) {
    let wav = PathBuf::from(&args[i + 1]);
    let opt = |k: &str| args.iter().position(|a| a == k).map(|j| args[j + 1].clone());
    let model = opt("--model").unwrap_or_else(|| models::discover().first().expect("no parakeet model found").path.clone());
    let speed: f32 = opt("--speed").map(|s| s.parse().unwrap()).unwrap_or(0.0);
    let (done_tx, done_rx) = crossbeam_channel::bounded(1);
    let t0 = std::time::Instant::now();
    let mut engine = engine::spawn(Box::new(move |e: Event| {
        match &e {
            Event::Partial { text } if text.is_empty() => {}
            Event::Partial { text } => eprintln!("  … {text}"),
            Event::Commit { t, text } => println!("[{}] {text}", engine::hms(*t)),
            Event::Level { .. } => {}
            Event::Saved { .. } => {
                eprintln!("{e:?}  (wall {:.1}s)", t0.elapsed().as_secs_f32());
                let _ = done_tx.send(());
            }
            other => eprintln!("{other:?}"),
        }
    }));
    engine.send(Cmd::Start(StartOpts {
        source: Source::Wav(wav, speed),
        model: model.clone(),
        model_label: model,
        language: opt("--lang"),
        title: "Simulation".into(),
        save_audio: opt("--continue").is_some(), // only appends to an existing WAV
        out_dir: std::env::temp_dir().join("transcribe-lecture-sim"),
        course: opt("--course").unwrap_or_default(),
        continue_path: opt("--continue").map(PathBuf::from),
    }));
    let _ = done_rx.recv();
    engine.shutdown();
}
