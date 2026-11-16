use lecture_scribe::engine::{self, Cmd, Event, Source, StartOpts};
use lecture_scribe::{audio, glossary, material, models, slides};
use serde_json::{json, Value};
use std::path::PathBuf;
use tao::{
    dpi::LogicalSize,
    event::{Event as TaoEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

/// The window's page: ui.html with the design system and slide renderer inlined.
fn ui() -> String {
    include_str!("ui.html")
        .replace("/*BUNDLE_CSS*/", slides::BUNDLE_CSS)
        .replace("/*TOKENS_CSS*/", slides::TOKENS_CSS)
        .replace("/*SLIDES_CSS*/", slides::SLIDES_CSS)
        .replace("/*SLIDES_JS*/", slides::SLIDES_JS)
}

static SLIDES_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn out_dir() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| dirs::home_dir().unwrap()).join("Lecture Transcripts")
}

fn settings_path() -> PathBuf {
    dirs::config_dir().unwrap().join("Lecture Scribe").join("settings.json")
}

fn load_settings() -> Value {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}))
}

fn init_payload() -> Value {
    json!({
        "type": "init",
        "models": models::discover(),
        "devices": audio::input_device_names(),
        "defaultDevice": audio::default_device_name(),
        "settings": load_settings(),
        "outDir": out_dir().to_string_lossy(),
        "recent": slides::recent(&out_dir()),
    })
}

fn main() {
    if std::env::var_os("LECTURE_SCRIBE_DEBUG").is_none() {
        transcribe_cpp::disable_logging();
    }
    transcribe_cpp::init_backends_default().expect("init ggml backends");
    let args: Vec<String> = std::env::args().collect();
    // Headless: `lecture-scribe --add-pdf file.pdf [--course "Protocolli e reti"]`
    if let Some(i) = args.iter().position(|a| a == "--add-pdf") {
        let course = args.iter().position(|a| a == "--course").map(|j| args[j + 1].clone()).unwrap_or_default();
        match material::add(&course, std::path::Path::new(&args[i + 1])) {
            Ok(v) => println!("added {v}"),
            Err(e) => eprintln!("error: {e}"),
        }
        return;
    }
    // Headless: `lecture-scribe --slides transcript.md [--course "Protocolli e reti"]`
    if let Some(i) = args.iter().position(|a| a == "--slides") {
        let course = args.iter().position(|a| a == "--course").map(|j| args[j + 1].clone()).unwrap_or_default();
        let rebuild = args.iter().any(|a| a == "--rebuild");
        match slides::generate(std::path::Path::new(&args[i + 1]), &course, rebuild) {
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
    if let Some(i) = args.iter().position(|a| a == "--simulate") {
        return simulate(&args, i);
    }

    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let emit_proxy = proxy.clone();
    let mut engine = engine::spawn(Box::new(move |e: Event| {
        if let Ok(s) = serde_json::to_string(&e) {
            let _ = emit_proxy.send_event(s);
        }
    }));
    let tx = engine.tx.clone();

    let window = WindowBuilder::new()
        .with_title("Lecture Scribe")
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
                // Slides: DeepSeek runs on its own thread; the result comes back as an event.
                "slides" => {
                    use std::sync::atomic::Ordering;
                    if SLIDES_BUSY.swap(true, Ordering::SeqCst) {
                        return;
                    }
                    let (path, course, proxy) = (PathBuf::from(s("path")), s("course"), ipc_proxy.clone());
                    let rebuild = msg["rebuild"].as_bool().unwrap_or(false);
                    std::thread::spawn(move || {
                        let r = slides::generate(&path, &course, rebuild);
                        SLIDES_BUSY.store(false, Ordering::SeqCst);
                        let ev = r.unwrap_or_else(|e| json!({ "type": "slidesError", "message": e, "path": path.to_string_lossy() }));
                        let _ = proxy.send_event(ev.to_string());
                    });
                }
                "slidesState" | "slidesUndo" => {
                    let path = PathBuf::from(s("path"));
                    let r = if msg["cmd"] == "slidesUndo" { slides::undo(&path) } else { slides::state(&path) };
                    let ev = r.unwrap_or_else(|e| json!({ "type": "slidesError", "message": e, "path": s("path") }));
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
                "loadTranscript" => {
                    let ev = slides::transcript_payload(&PathBuf::from(s("path")))
                        .unwrap_or_else(|e| json!({ "type": "error", "message": e }));
                    let _ = ipc_proxy.send_event(ev.to_string());
                }
                "saveSettings" => {
                    let p = settings_path();
                    let _ = std::fs::create_dir_all(p.parent().unwrap());
                    let _ = std::fs::write(p, msg["settings"].to_string());
                }
                _ => {}
            }
        })
        .build(&window)
        .expect("webview");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            TaoEvent::UserEvent(js) => {
                let _ = webview.evaluate_script(&format!("window.onEngine({js})"));
            }
            TaoEvent::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                engine.shutdown(); // finishes and saves any running recording
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
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

/// Headless test: `lecture-scribe --simulate file.wav [--model path] [--speed N] [--lang it]`
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
        save_audio: false,
        out_dir: std::env::temp_dir().join("lecture-scribe-sim"),
        course: opt("--course").unwrap_or_default(),
    }));
    let _ = done_rx.recv();
    engine.shutdown();
}
