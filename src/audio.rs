//! Microphone capture (cpal) → mono 16 kHz f32 chunks on a channel.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;

pub const TARGET_RATE: u32 = 16_000;

pub enum AudioMsg {
    Samples(Vec<f32>),
    /// The capture device failed (unplugged, permission revoked, ...).
    DeviceError(String),
    /// A file source reached its end.
    EndOfInput,
}

pub fn input_device_names() -> Vec<String> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    if let Ok(devs) = host.input_devices() {
        for d in devs {
            if let Ok(desc) = d.description() {
                names.push(desc.name().to_string());
            }
        }
    }
    names
}

pub fn default_device_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.description().ok())
        .map(|d| d.name().to_string())
}

/// A running capture. Dropping it stops the microphone.
pub struct Capture {
    _stream: cpal::Stream,
    pub device_name: String,
}

pub fn start_capture(device: Option<&str>, tx: Sender<AudioMsg>) -> Result<Capture, String> {
    let host = cpal::default_host();
    let dev = match device {
        Some(want) if !want.is_empty() => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.description().map(|x| x.name() == want).unwrap_or(false))
            .or_else(|| host.default_input_device()),
        _ => host.default_input_device(),
    }
    .ok_or("No microphone found")?;
    let device_name = dev
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "Microphone".into());

    let supported = dev.default_input_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config = supported.config();
    let channels = config.channels as usize;
    let mut rs = Resampler::new(config.sample_rate);

    let err_tx = tx.clone();
    let on_err = move |e: cpal::Error| {
        let _ = err_tx.send(AudioMsg::DeviceError(e.to_string()));
    };

    macro_rules! build {
        ($t:ty, $conv:expr) => {{
            let tx = tx.clone();
            dev.build_input_stream::<$t, _, _>(
                config.clone(),
                move |data: &[$t], _| {
                    let mono: Vec<f32> = data
                        .chunks(channels)
                        .map(|fr| fr.iter().map(|&s| $conv(s)).sum::<f32>() / channels as f32)
                        .collect();
                    let out = rs.process(&mono);
                    if !out.is_empty() {
                        let _ = tx.send(AudioMsg::Samples(out));
                    }
                },
                on_err,
                None,
            )
        }};
    }

    let stream = match format {
        cpal::SampleFormat::F32 => build!(f32, |s: f32| s),
        cpal::SampleFormat::I16 => build!(i16, |s: i16| s as f32 / 32768.0),
        cpal::SampleFormat::I32 => build!(i32, |s: i32| s as f32 / 2147483648.0),
        cpal::SampleFormat::U16 => build!(u16, |s: u16| (s as f32 - 32768.0) / 32768.0),
        f => return Err(format!("Unsupported microphone sample format {f}")),
    }
    .map_err(|e| {
        let msg = e.to_string();
        if msg.to_lowercase().contains("permission") {
            "Microphone permission denied. Allow it in System Settings → Privacy & Security → Microphone.".to_string()
        } else {
            msg
        }
    })?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Capture { _stream: stream, device_name })
}

/// Band-limited (windowed-sinc) resampler to 16 kHz.
pub struct Resampler {
    step: f64,
    cutoff: f64,
    half: usize,
    buf: Vec<f32>,
    pos: f64,
    passthrough: bool,
}

impl Resampler {
    pub fn new(in_rate: u32) -> Self {
        let step = in_rate as f64 / TARGET_RATE as f64;
        let cutoff = 0.5 * (1.0 / step).min(1.0) * 0.92; // cycles per input sample
        let half = (16.0 * step.max(1.0)).ceil() as usize;
        Resampler { step, cutoff, half, buf: Vec::new(), pos: 0.0, passthrough: in_rate == TARGET_RATE }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.passthrough {
            return input.to_vec();
        }
        self.buf.extend_from_slice(input);
        let mut out = Vec::with_capacity((input.len() as f64 / self.step) as usize + 2);
        let h = self.half as f64;
        while self.pos + h < self.buf.len() as f64 {
            let center = self.pos;
            let lo = (center - h).ceil().max(0.0) as usize;
            let hi = (center + h).floor() as usize;
            let mut acc = 0.0f64;
            let mut norm = 0.0f64;
            for i in lo..=hi.min(self.buf.len() - 1) {
                let x = i as f64 - center;
                let s = if x.abs() < 1e-9 {
                    2.0 * self.cutoff
                } else {
                    (2.0 * std::f64::consts::PI * self.cutoff * x).sin() / (std::f64::consts::PI * x)
                };
                let w = 0.5 + 0.5 * (std::f64::consts::PI * x / (h + 1.0)).cos(); // Hann
                let k = s * w;
                acc += k * self.buf[i] as f64;
                norm += k;
            }
            out.push(if norm.abs() > 1e-9 { (acc / norm) as f32 } else { 0.0 });
            self.pos += self.step;
        }
        // Drop input we no longer need for the kernel's left side.
        let keep_from = (self.pos - h).floor().max(0.0) as usize;
        if keep_from > 0 {
            self.buf.drain(..keep_from);
            self.pos -= keep_from as f64;
        }
        out
    }
}
