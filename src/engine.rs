//! Recording + transcription worker.
//!
//! Audio arrives as 16 kHz mono f32. It is split into utterances at natural
//! pauses (energy-based, adaptive noise floor). Each finished utterance is
//! transcribed with Parakeet. The transcript file is rewritten (atomically)
//! on every change, including the sentence still being spoken, so it is
//! always up to date on disk and nothing is lost if the app closes or crashes. While an utterance is
//! still in progress it is re-transcribed about once a second to show live
//! "partial" text.

use crate::audio::{self, AudioMsg, Capture, Resampler, TARGET_RATE};
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use serde::Serialize;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use transcribe_cpp::{Model, RunOptions, Session};

const FRAME: usize = 480; // 30 ms at 16 kHz
const FRAME_S: f64 = FRAME as f64 / TARGET_RATE as f64;
const fn frames(secs: f64) -> usize {
    (secs / FRAME_S) as usize
}

pub enum Source {
    Mic(Option<String>),
    /// Testing aid: play a WAV file into the engine `speed`× faster than real time.
    Wav(PathBuf, f32),
}

pub struct StartOpts {
    pub source: Source,
    pub model: String,
    pub model_label: String,
    pub language: Option<String>,
    pub title: String,
    pub save_audio: bool,
    pub out_dir: PathBuf,
    /// Course name: picks the glossary whose corrections are applied live.
    pub course: String,
}

pub enum Cmd {
    Preload(String),
    Start(StartOpts),
    Pause,
    Resume,
    Stop,
    Shutdown,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    ModelLoading { path: String },
    ModelReady { path: String, ms: u64 },
    State { state: &'static str, device: Option<String> },
    Partial { text: String },
    Commit { t: f64, text: String },
    Level { db: f32 },
    Notice { message: String },
    Error { message: String },
    Saved { transcript: String, audio: Option<String> },
    Autosaved { path: String },
}

pub type Emit = Box<dyn Fn(Event) + Send>;

pub struct EngineHandle {
    pub tx: Sender<Cmd>,
    join: Option<JoinHandle<()>>,
}

impl EngineHandle {
    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

pub fn spawn(emit: Emit) -> EngineHandle {
    let (tx, rx) = unbounded();
    let join = std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || Worker::new(emit).run(rx))
        .expect("spawn engine");
    EngineHandle { tx, join: Some(join) }
}

struct Rec {
    session: Session,
    run_opts: RunOptions,
    transcript_path: PathBuf,
    /// Finished transcript (markdown); the file on disk is `body` + the live partial line.
    body: String,
    wav: Option<(PathBuf, hound::WavWriter<BufWriter<File>>)>,
    capture: Option<Capture>,
    device_pref: Option<String>,
    is_mic: bool,
    paused: bool,
    retry_at: Option<Instant>,
    caffeinate: Option<std::process::Child>,
    // utterance state
    utt: Vec<f32>,
    utt_frames: Vec<(f32, bool)>, // (dBFS, is_speech)
    utt_start: u64,
    floor_db: f32,
    since_partial: usize,
    last_partial: String,
    level_acc: (f64, usize),
    last_wav_flush: Instant,
    fix: crate::glossary::Corrector,
}

struct Worker {
    emit: Emit,
    loaded: Option<(String, Model)>,
    audio_tx: Sender<AudioMsg>,
    audio_rx: Receiver<AudioMsg>,
    rec: Option<Rec>,
}

impl Worker {
    fn new(emit: Emit) -> Self {
        let (audio_tx, audio_rx) = unbounded();
        Worker { emit, loaded: None, audio_tx, audio_rx, rec: None }
    }

    fn run(mut self, cmd_rx: Receiver<Cmd>) {
        let audio_rx = self.audio_rx.clone();
        loop {
            select! {
                recv(cmd_rx) -> c => match c {
                    Ok(Cmd::Shutdown) | Err(_) => { self.stop(); return; }
                    Ok(c) => self.on_cmd(c),
                },
                recv(audio_rx) -> m => if let Ok(m) = m { self.on_audio(m) },
                default(Duration::from_millis(250)) => {},
            }
            self.tick();
        }
    }

    fn ensure_model(&mut self, path: &str) -> Result<Model, String> {
        if let Some((p, m)) = &self.loaded {
            if p == path {
                return Ok(m.clone());
            }
        }
        (self.emit)(Event::ModelLoading { path: path.into() });
        let t0 = Instant::now();
        self.loaded = None; // free the old model first
        let m = Model::load(path).map_err(|e| format!("Could not load model: {e}"))?;
        (self.emit)(Event::ModelReady { path: path.into(), ms: t0.elapsed().as_millis() as u64 });
        self.loaded = Some((path.into(), m.clone()));
        Ok(m)
    }

    fn on_cmd(&mut self, c: Cmd) {
        match c {
            Cmd::Preload(p) => {
                if self.rec.is_none() {
                    if let Err(e) = self.ensure_model(&p) {
                        (self.emit)(Event::Error { message: e });
                    }
                }
            }
            Cmd::Start(o) => {
                if self.rec.is_some() {
                    return;
                }
                if let Err(e) = self.start(o) {
                    (self.emit)(Event::Error { message: e });
                    (self.emit)(Event::State { state: "idle", device: None });
                }
            }
            Cmd::Pause => self.pause(),
            Cmd::Resume => self.resume(),
            Cmd::Stop => self.stop(),
            Cmd::Shutdown => unreachable!(),
        }
    }

    fn start(&mut self, o: StartOpts) -> Result<(), String> {
        let model = self.ensure_model(&o.model)?;
        let session = model.session().map_err(|e| e.to_string())?;
        while self.audio_rx.try_recv().is_ok() {} // discard stale audio

        std::fs::create_dir_all(&o.out_dir).map_err(|e| e.to_string())?;
        let now = chrono::Local::now();
        let title = if o.title.trim().is_empty() { "Lecture".to_string() } else { o.title.trim().to_string() };
        let safe: String = title
            .chars()
            .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
            .collect();
        let stem = unique_stem(&o.out_dir, &format!("{} {}", now.format("%Y-%m-%d %H.%M"), safe));
        let transcript_path = o.out_dir.join(format!("{stem}.md"));
        let body = format!("# {title}\n\n_{}_ · {}\n\n", now.format("%A %-d %B %Y, %H:%M"), o.model_label);
        write_atomic(&transcript_path, &body).map_err(|e| format!("Could not create transcript file: {e}"))?;

        let wav = if o.save_audio {
            let p = o.out_dir.join(format!("{stem}.wav"));
            let spec = hound::WavSpec { channels: 1, sample_rate: TARGET_RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
            Some((p.clone(), hound::WavWriter::create(&p, spec).map_err(|e| e.to_string())?))
        } else {
            None
        };

        let (capture, device_pref, is_mic) = match o.source {
            Source::Mic(dev) => {
                let cap = audio::start_capture(dev.as_deref(), self.audio_tx.clone())?;
                (Some(cap), dev, true)
            }
            Source::Wav(path, speed) => {
                spawn_wav_feeder(path, speed, self.audio_tx.clone())?;
                (None, None, false)
            }
        };
        let device = capture.as_ref().map(|c| c.device_name.clone()).or(Some("file".into()));

        let caffeinate = if is_mic {
            std::process::Command::new("/usr/bin/caffeinate")
                .args(["-i", "-w", &std::process::id().to_string()])
                .spawn()
                .ok()
        } else {
            None
        };

        self.rec = Some(Rec {
            session,
            run_opts: RunOptions { language: o.language, ..Default::default() },
            fix: crate::glossary::Corrector::new(&o.course),
            transcript_path,
            body,
            wav,
            capture,
            device_pref,
            is_mic,
            paused: false,
            retry_at: None,
            caffeinate,
            utt: Vec::new(),
            utt_frames: Vec::new(),
            utt_start: 0,
            floor_db: -60.0,
            since_partial: 0,
            last_partial: String::new(),
            level_acc: (0.0, 0),
            last_wav_flush: Instant::now(),
        });
        (self.emit)(Event::State { state: "recording", device });
        Ok(())
    }

    fn pause(&mut self) {
        let Some(r) = self.rec.as_mut() else { return };
        if r.paused {
            return;
        }
        r.paused = true;
        r.capture = None; // release the microphone
        r.retry_at = None;
        let n = r.utt_frames.len();
        self.commit(n);
        if let Some(r) = self.rec.as_mut() {
            r.utt.clear();
            r.utt_frames.clear();
        }
        (self.emit)(Event::State { state: "paused", device: None });
    }

    fn resume(&mut self) {
        let Some(r) = self.rec.as_mut() else { return };
        if !r.paused {
            return;
        }
        while self.audio_rx.try_recv().is_ok() {}
        r.paused = false;
        if r.is_mic {
            match audio::start_capture(r.device_pref.as_deref(), self.audio_tx.clone()) {
                Ok(c) => {
                    let name = c.device_name.clone();
                    r.capture = Some(c);
                    (self.emit)(Event::State { state: "recording", device: Some(name) });
                }
                Err(e) => {
                    r.retry_at = Some(Instant::now() + Duration::from_secs(2));
                    (self.emit)(Event::Error { message: e });
                    (self.emit)(Event::State { state: "recording", device: None });
                }
            }
        }
    }

    fn stop(&mut self) {
        if self.rec.is_none() {
            return;
        }
        // Process any audio still queued, then flush the final utterance.
        if let Some(r) = self.rec.as_mut() {
            r.capture = None;
        }
        while let Ok(m) = self.audio_rx.try_recv() {
            if let AudioMsg::Samples(s) = m {
                self.on_samples(&s);
            }
        }
        let n = self.rec.as_ref().map(|r| r.utt_frames.len()).unwrap_or(0);
        self.commit(n);
        let mut r = self.rec.take().unwrap();
        // Include the sub-frame tail too, if any (only matters for the WAV, already written).
        let audio = r.wav.take().map(|(p, w)| {
            let _ = w.finalize();
            p.to_string_lossy().to_string()
        });
        let _ = write_atomic(&r.transcript_path, &r.body);
        if let Some(mut c) = r.caffeinate.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        (self.emit)(Event::Partial { text: String::new() });
        (self.emit)(Event::Saved { transcript: r.transcript_path.to_string_lossy().to_string(), audio });
        (self.emit)(Event::State { state: "idle", device: None });
    }

    fn tick(&mut self) {
        let Some(r) = self.rec.as_mut() else { return };
        if let Some(at) = r.retry_at {
            if Instant::now() >= at && !r.paused {
                match audio::start_capture(r.device_pref.as_deref(), self.audio_tx.clone()) {
                    Ok(c) => {
                        let name = c.device_name.clone();
                        r.capture = Some(c);
                        r.retry_at = None;
                        (self.emit)(Event::Notice { message: format!("Recording again from {name}.") });
                        (self.emit)(Event::State { state: "recording", device: Some(name) });
                    }
                    Err(_) => r.retry_at = Some(Instant::now() + Duration::from_secs(2)),
                }
            }
        }
    }

    fn on_audio(&mut self, m: AudioMsg) {
        match m {
            AudioMsg::Samples(s) => {
                // Coalesce whatever else is queued so we never fall behind.
                let mut s = s;
                let mut other = None;
                while let Ok(m) = self.audio_rx.try_recv() {
                    match m {
                        AudioMsg::Samples(more) => s.extend(more),
                        m => {
                            other = Some(m);
                            break;
                        }
                    }
                }
                for c in s.chunks(1600) {
                    self.on_samples(c);
                    self.segment();
                }
                if let Some(m) = other {
                    self.on_audio(m);
                }
            }
            AudioMsg::DeviceError(e) => {
                if let Some(r) = self.rec.as_mut() {
                    if r.capture.is_some() {
                        r.capture = None;
                        r.retry_at = Some(Instant::now() + Duration::from_secs(1));
                        (self.emit)(Event::Notice { message: format!("Microphone problem ({e}). Reconnecting…") });
                    }
                }
            }
            AudioMsg::EndOfInput => self.stop(),
        }
    }

    fn on_samples(&mut self, s: &[f32]) {
        let Some(r) = self.rec.as_mut() else { return };
        if r.paused {
            return;
        }
        if let Some((_, w)) = r.wav.as_mut() {
            for &x in s {
                let _ = w.write_sample((x.clamp(-1.0, 1.0) * 32767.0) as i16);
            }
            if r.last_wav_flush.elapsed() > Duration::from_secs(2) {
                let _ = w.flush(); // keeps the file playable if we crash
                r.last_wav_flush = Instant::now();
            }
        }
        r.utt.extend_from_slice(s);
        r.since_partial += s.len();

        // level meter (~10 updates/s)
        for &x in s {
            r.level_acc.0 += (x * x) as f64;
        }
        r.level_acc.1 += s.len();
        if r.level_acc.1 >= 1600 {
            let db = 10.0 * ((r.level_acc.0 / r.level_acc.1 as f64) + 1e-12).log10();
            r.level_acc = (0.0, 0);
            (self.emit)(Event::Level { db: db as f32 });
        }

        // frame analysis with adaptive noise floor
        while (r.utt_frames.len() + 1) * FRAME <= r.utt.len() {
            let i = r.utt_frames.len() * FRAME;
            let e: f64 = r.utt[i..i + FRAME].iter().map(|&x| (x * x) as f64).sum::<f64>() / FRAME as f64;
            let db = (10.0 * (e + 1e-12).log10()) as f32;
            if db < r.floor_db {
                r.floor_db = 0.8 * r.floor_db + 0.2 * db;
            } else {
                r.floor_db += 0.015;
            }
            r.floor_db = r.floor_db.clamp(-85.0, -25.0);
            let speech = db > r.floor_db + 9.0 && db > -62.0;
            r.utt_frames.push((db, speech));
        }
    }

    /// Decide whether the current utterance should be finished.
    fn segment(&mut self) {
        let Some(r) = self.rec.as_mut() else { return };
        let n = r.utt_frames.len();
        let speech_frames = r.utt_frames.iter().filter(|f| f.1).count();
        let trailing_sil = r.utt_frames.iter().rev().take_while(|f| !f.1).count();

        if speech_frames < 6 {
            // Nothing but silence/noise: drop it (keep a short lead-in).
            if n > frames(3.0) {
                let drop = n - frames(0.3);
                r.utt.drain(..drop * FRAME);
                r.utt_frames.drain(..drop);
                r.utt_start += (drop * FRAME) as u64;
                if !r.last_partial.is_empty() {
                    r.last_partial.clear();
                    (self.emit)(Event::Partial { text: String::new() });
                }
            }
            return;
        }

        let dur = n as f64 * FRAME_S;
        if (dur >= 6.0 && trailing_sil >= frames(0.45))
            || (dur >= 2.0 && trailing_sil >= frames(0.9))
            || trailing_sil >= frames(2.0)
        {
            self.commit(n);
        } else if dur >= 24.0 {
            // Long stretch without a clear pause: cut at the quietest point of the last 6 s.
            let lo = n - frames(6.0);
            let (k, _) = r.utt_frames[lo..]
                .iter()
                .enumerate()
                .min_by(|a, b| a.1 .0.total_cmp(&b.1 .0))
                .map(|(i, f)| (lo + i + 1, f.0))
                .unwrap();
            self.commit(k);
        } else if r.since_partial >= TARGET_RATE as usize && self.audio_rx.is_empty() {
            r.since_partial = 0;
            let n_audio = n * FRAME;
            match r.session.run(&r.utt[..n_audio], &r.run_opts) {
                Ok(t) => {
                    let text = r.fix.apply(t.text.trim());
                    if text != r.last_partial {
                        r.last_partial = text.clone();
                        let t = r.utt_start as f64 / TARGET_RATE as f64;
                        let snapshot = if text.is_empty() {
                            r.body.clone()
                        } else {
                            format!("{}**[{}]** {} …\n", r.body, hms(t), text)
                        };
                        if write_atomic(&r.transcript_path, &snapshot).is_ok() {
                            (self.emit)(Event::Autosaved { path: r.transcript_path.to_string_lossy().to_string() });
                        }
                        (self.emit)(Event::Partial { text });
                    }
                }
                Err(e) => (self.emit)(Event::Error { message: format!("Transcription error: {e}") }),
            }
        }
    }

    /// Transcribe the first `k` frames of the utterance and append them to the transcript.
    fn commit(&mut self, k: usize) {
        let Some(r) = self.rec.as_mut() else { return };
        let k = k.min(r.utt_frames.len());
        if k == 0 {
            return;
        }
        let has_speech = r.utt_frames[..k].iter().filter(|f| f.1).count() >= 6;
        let t = r.utt_start as f64 / TARGET_RATE as f64;
        if has_speech {
            match r.session.run(&r.utt[..k * FRAME], &r.run_opts) {
                Ok(res) => {
                    let text = r.fix.apply(res.text.trim());
                    if !text.is_empty() {
                        r.body.push_str(&format!("**[{}]** {}\n\n", hms(t), text));
                        (self.emit)(Event::Commit { t, text });
                    }
                }
                Err(e) => (self.emit)(Event::Error { message: format!("Transcription error: {e}") }),
            }
        }
        // Persist now (this also replaces the "…" live line in the file).
        match write_atomic(&r.transcript_path, &r.body) {
            Ok(()) => (self.emit)(Event::Autosaved { path: r.transcript_path.to_string_lossy().to_string() }),
            Err(e) => (self.emit)(Event::Error { message: format!("Could not save transcript: {e}") }),
        }
        r.utt.drain(..k * FRAME);
        r.utt_frames.drain(..k);
        r.utt_start += (k * FRAME) as u64;
        r.since_partial = 0;
        r.last_partial.clear();
        (self.emit)(Event::Partial { text: String::new() });
    }
}

pub fn hms(t: f64) -> String {
    let s = t as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

/// Write via a temp file + rename so the transcript is never half-written.
fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)
}

fn unique_stem(dir: &Path, stem: &str) -> String {
    let mut s = stem.to_string();
    let mut i = 2;
    while dir.join(format!("{s}.md")).exists() {
        s = format!("{stem} ({i})");
        i += 1;
    }
    s
}

fn spawn_wav_feeder(path: PathBuf, speed: f32, tx: Sender<AudioMsg>) -> Result<(), String> {
    let mut reader = hound::WavReader::open(&path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let ch = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.unwrap_or(0) as f32 / scale).collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
    };
    let mono: Vec<f32> = raw.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect();
    let pcm = Resampler::new(spec.sample_rate).process(&mono);
    std::thread::spawn(move || {
        for chunk in pcm.chunks(1600) {
            if tx.send(AudioMsg::Samples(chunk.to_vec())).is_err() {
                return;
            }
            if speed > 0.0 {
                std::thread::sleep(Duration::from_secs_f32(0.1 / speed));
            }
        }
        let _ = tx.send(AudioMsg::EndOfInput);
    });
    Ok(())
}
