//! End-to-end engine tests: real Parakeet transcription of speech made with macOS `say`.
//! They need the model on this Mac, so they're `#[ignore]`d; `scripts/verify.sh` runs them
//! (`cargo test --release --test engine -- --ignored`). Everything is written to a temp folder.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;
use transcribe_lecture::engine::{self, Cmd, Event, Source, StartOpts};
use transcribe_lecture::slides::parse_transcript;

const SPEECH: &str = "Good morning everyone. [[slnc 1200]] Today we talk about gradient descent. [[slnc 1200]] \
    The learning rate controls the size of each step. [[slnc 1200]] Stochastic gradient descent uses one example at a time. \
    [[slnc 1200]] Next week there is a lab, so bring your laptops. [[slnc 1500]]";

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lecture-scribe-engine-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A 16 kHz mono WAV of SPEECH, spoken by the system voice.
fn speech_wav(d: &Path) -> PathBuf {
    let (aiff, wav) = (d.join("speech.aiff"), d.join("speech.wav"));
    assert!(Command::new("say").args(["-v", "Samantha", "-o"]).arg(&aiff).arg(SPEECH).status().unwrap().success()
        || Command::new("say").arg("-o").arg(&aiff).arg(SPEECH).status().unwrap().success(), "say failed");
    assert!(Command::new("afconvert").args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"]).arg(&aiff).arg(&wav).status().unwrap().success());
    wav
}

fn model() -> String {
    transcribe_lecture::models::discover().first().expect("no Parakeet model on this Mac: run the app's setup first").path.clone()
}

/// `speed`: how many times faster than real time the WAV is fed (0: all at once).
fn opts(wav: &Path, out: &Path, continue_path: Option<PathBuf>, speed: f32) -> StartOpts {
    StartOpts {
        source: Source::Wav(wav.to_path_buf(), speed),
        model: model(),
        model_label: "test".into(),
        language: Some("en".into()),
        title: "Engine Test".into(),
        save_audio: false,
        out_dir: out.to_path_buf(),
        course: String::new(),
        continue_path,
    }
}

/// Runs one recording to the end of the WAV. `on_commit(n, path, engine)` is called after each
/// finished line. Returns the transcript path and every event.
fn record(o: StartOpts, mut on_commit: impl FnMut(usize, &Path, &engine::EngineHandle)) -> (PathBuf, Vec<Event>) {
    transcribe_cpp::disable_logging();
    let (tx, rx) = mpsc::channel();
    let mut eng = engine::spawn(Box::new(move |e| { let _ = tx.send(e); }));
    eng.send(Cmd::Start(o));
    let (mut path, mut events, mut n) = (PathBuf::new(), Vec::new(), 0);
    loop {
        let e = rx.recv_timeout(Duration::from_secs(120)).expect("engine stalled");
        match &e {
            Event::Autosaved { path: p } => path = PathBuf::from(p),
            Event::Commit { .. } => { n += 1; on_commit(n, &path, &eng); }
            Event::Error { message } => panic!("engine error: {message}"),
            _ => {}
        }
        let done = matches!(e, Event::Saved { .. });
        events.push(e);
        if done { break; }
    }
    eng.shutdown();
    (path, events)
}

fn lines(p: &Path) -> Vec<(String, String)> {
    parse_transcript(&std::fs::read_to_string(p).unwrap()).1
}

#[test]
#[ignore]
fn transcribes_speech() {
    let d = dir("plain");
    let (p, _) = record(opts(&speech_wav(&d), &d, None, 0.0), |_, _, _| {});
    let l = lines(&p);
    let all = l.iter().map(|x| x.1.to_lowercase()).collect::<Vec<_>>().join(" ");
    assert!(l.len() >= 3, "too few lines: {l:?}");
    for w in ["gradient", "learning rate", "laptops"] {
        assert!(all.contains(w), "{w:?} not heard in {all:?}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Hand edits made while recording go into the lecture the engine holds in memory, so they
/// survive its next rewrite of the file; new lines keep arriving after them.
#[test]
#[ignore]
fn edits_during_a_recording_are_kept() {
    let d = dir("edit");
    let wav = speech_wav(&d);
    // 4x real time, so the edits arrive while the engine is still recording.
    let (p, events) = record(opts(&wav, &d, None, 4.0), |n, path, eng| {
        if n == 2 {
            eng.send(Cmd::EditLine { path: path.into(), op: "set".into(), i: 0, text: "EDITED FIRST LINE".into() });
            eng.send(Cmd::EditLine { path: path.into(), op: "insert".into(), i: 1, text: "MY OWN NOTE".into() });
        }
    });
    let l = lines(&p);
    assert_eq!(l[0].1, "EDITED FIRST LINE", "{l:?}");
    assert_eq!(l[1].1, "MY OWN NOTE", "{l:?}");
    assert_eq!(l[1].0, l[0].0, "a new line takes the time of the line above");
    assert!(l.len() >= 4, "lines after the edit are missing: {l:?}");
    // Both edits were applied by the recording (before it saved), not to the file afterwards.
    let saved = events.iter().position(|e| matches!(e, Event::Saved { .. })).unwrap();
    let edited = events[..saved].iter().filter(|e| matches!(e, Event::LineEdited { .. })).count();
    assert_eq!(edited, 2, "one LineEdited event per edit, during the recording");

    // Continue the same lecture: edits stay, new lines come after the "Continued" note.
    let before = l.len();
    let (p2, _) = record(opts(&wav, &d, Some(p.clone()), 0.0), |_, _, _| {});
    assert_eq!(p2, p);
    let l2 = lines(&p);
    assert!(l2.len() > before && l2[0].1 == "EDITED FIRST LINE" && l2[1].1 == "MY OWN NOTE", "{l2:?}");
    assert!(std::fs::read_to_string(&p).unwrap().contains("_Continued "));
    let _ = std::fs::remove_dir_all(&d);
}
