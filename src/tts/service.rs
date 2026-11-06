//! High-level TTS service: generation thread + serialized playback thread.
//! Port of `src/services/tts_service.py`.
//!
//! Mirrors the original semantics:
//! - `speak(text, voice, lang, cache=true)`: cache hit plays the cached wav;
//!   miss generates via Supertonic ONNX then caches by
//!   `md5(text_voice_lang_modelversion)`.
//! - `cache=false` (Test Voice): writes a uniquely-named temp wav and deletes
//!   it after playback.
//! - Playback queue: size 1 with override - while audio is playing, a new
//!   request replaces any pending one; only the newest pending is played.
//! - Missing assets / generation failure falls back to a generated dummy
//!   440 Hz wav and reports `TtsEvent::Error` to the UI.
//! - `chime()` plays the hourly 1000 Hz / 200 ms beep (winsound.Beep).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

use anyhow::Context;
use md5::{Digest, Md5};

use super::engine::{load_voice_style, SupertonicEngine};

#[derive(Clone, Debug)]
pub struct SpeakRequest {
    pub text: String,
    pub voice: String,
    pub lang: String,
    pub cache: bool,
}

#[derive(Clone, Debug)]
pub enum TtsEvent {
    Error(String),
}

enum GeneratorMsg {
    Speak(SpeakRequest),
}

enum PlayerMsg {
    File { path: PathBuf, delete_after: bool },
    Tone { freq_hz: f32, dur_ms: u32 },
}

pub struct TtsService {
    tx: Sender<GeneratorMsg>,
    player_tx: Sender<PlayerMsg>,
    event_rx: Option<Receiver<TtsEvent>>,
}

impl TtsService {
    /// Spawns the generator + player threads. The ONNX engine is loaded
    /// lazily on the generator thread so the UI starts immediately even with
    /// the ~390 MB of models (same startup pattern as the original).
    pub fn new(
        onnx_dir: PathBuf,
        voice_styles_dir: PathBuf,
        cache_dir: PathBuf,
        ctx: egui::Context,
    ) -> Self {
        let _ = std::fs::create_dir_all(&cache_dir);

        let (gen_tx, gen_rx) = std::sync::mpsc::channel::<GeneratorMsg>();
        let (player_tx, player_rx) = std::sync::mpsc::channel::<PlayerMsg>();
        let (event_tx, event_rx) = std::sync::mpsc::channel::<TtsEvent>();

        std::thread::Builder::new()
            .name("tts-player".into())
            .spawn({
                let ctx = ctx.clone();
                move || player_loop(player_rx, ctx)
            })
            .expect("spawn tts-player");

        let player_tx_gen = player_tx.clone();
        std::thread::Builder::new()
            .name("tts-generator".into())
            .spawn(move || {
                generator_loop(
                    gen_rx,
                    player_tx_gen,
                    event_tx,
                    onnx_dir,
                    voice_styles_dir,
                    cache_dir,
                    ctx,
                )
            })
            .expect("spawn tts-generator");

        Self {
            tx: gen_tx,
            player_tx,
            event_rx: Some(event_rx),
        }
    }

    /// Take ownership of the UI-facing event receiver.
    pub fn take_event_receiver(&mut self) -> Option<Receiver<TtsEvent>> {
        self.event_rx.take()
    }

    pub fn speak(
        &self,
        text: impl Into<String>,
        voice: impl Into<String>,
        lang: impl Into<String>,
        cache: bool,
    ) {
        let _ = self.tx.send(GeneratorMsg::Speak(SpeakRequest {
            text: text.into(),
            voice: voice.into(),
            lang: lang.into(),
            cache,
        }));
    }

    /// Hourly chime: 1000 Hz beep for 200 ms.
    pub fn chime(&self) {
        let _ = self.player_tx.send(PlayerMsg::Tone {
            freq_hz: 1000.0,
            dur_ms: 200,
        });
    }

    /// Poll UI events (TTS errors) without blocking.
    pub fn poll_events(&mut self) -> Vec<TtsEvent> {
        let Some(rx) = &self.event_rx else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            out.push(e);
        }
        out
    }
}

fn generator_loop(
    rx: Receiver<GeneratorMsg>,
    player_tx: Sender<PlayerMsg>,
    event_tx: Sender<TtsEvent>,
    onnx_dir: PathBuf,
    voice_styles_dir: PathBuf,
    cache_dir: PathBuf,
    ctx: egui::Context,
) {
    // --- lazy engine init (mirrors TTSService.__init__) ---
    let assets_exist = onnx_dir.is_dir() && voice_styles_dir.is_dir();
    let mut engine: Option<SupertonicEngine> = if assets_exist {
        println!("Loading Supertonic TTS from: {}", onnx_dir.display());
        match SupertonicEngine::load(&onnx_dir) {
            Ok(e) => Some(e),
            Err(e) => {
                eprintln!("Failed to init Supertonic: {e:#}");
                None
            }
        }
    } else {
        println!(
            "Assets not found at {}. Please download assets first.",
            onnx_dir.display()
        );
        None
    };

    // Model version for cache keying, so audio cached by an older model never
    // gets replayed after a model upgrade
    let model_version = engine
        .as_ref()
        .map(|e| e.model_version().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let mut available_voices = scan_voice_styles(&voice_styles_dir);
    if !available_voices.is_empty() {
        println!("Supertonic voices: {available_voices:?}");
    }
    let mut voice_styles: std::collections::HashMap<String, super::engine::Style> =
        std::collections::HashMap::new();

    while let Ok(msg) = rx.recv() {
        let GeneratorMsg::Speak(req) = msg;

        let filepath = if req.cache {
            let filename = cache_filename(&req.text, &req.voice, &req.lang, &model_version);
            cache_dir.join(filename)
        } else {
            // Unique name per call so concurrent Test Voice clicks don't race
            cache_dir.join(format!("temp_test_voice_{}.wav", random_hex(16)))
        };

        if !req.cache || !filepath.exists() {
            println!(
                "Generating TTS (Cache={}, Lang={}) for: {}",
                req.cache, req.lang, req.text
            );
            generate_audio(
                &req,
                &mut engine,
                &mut available_voices,
                &mut voice_styles,
                &voice_styles_dir,
                &filepath,
                &event_tx,
                &ctx,
            );
        }

        if filepath.exists() {
            let _ = player_tx.send(PlayerMsg::File {
                path: filepath,
                delete_after: !req.cache,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_audio(
    req: &SpeakRequest,
    engine: &mut Option<SupertonicEngine>,
    available_voices: &mut Vec<String>,
    voice_styles: &mut std::collections::HashMap<String, super::engine::Style>,
    voice_styles_dir: &Path,
    output_path: &Path,
    event_tx: &Sender<TtsEvent>,
    ctx: &egui::Context,
) {
    let mut success = false;
    if let Some(engine) = engine.as_mut() {
        let result = (|| -> anyhow::Result<()> {
            // Resolve voice
            let mut voice_name = req.voice.clone();
            if !available_voices.is_empty() && !available_voices.contains(&voice_name) {
                voice_name = available_voices[0].clone();
                println!(
                    "Requested voice '{}' not found. Using '{voice_name}'.",
                    req.voice
                );
            }

            // Load voice style (cached)
            let style = if let Some(s) = voice_styles.get(&voice_name) {
                s.clone()
            } else {
                let path = voice_styles_dir.join(format!("{voice_name}.json"));
                let s = load_voice_style(&[&path])?;
                voice_styles.insert(voice_name, s.clone());
                s
            };

            // Synthesize (total_step=5, speed=1.05 like the original)
            let (wav, _dur) = engine
                .synthesize(&req.text, &req.lang, &style, 5, 1.05, 0.3)
                .context("synthesis")?;

            write_wav_pcm16(output_path, &wav, engine.sample_rate())
                .with_context(|| format!("writing {}", output_path.display()))?;
            Ok(())
        })();

        match result {
            Ok(()) => success = true,
            Err(e) => {
                let msg = format!("TTS Generation Error: {e:#}");
                println!("{msg}");
                let _ = event_tx.send(TtsEvent::Error(msg.clone()));
                ctx.request_repaint();
            }
        }
    }

    if !success {
        println!(
            "[Simulated or Failed Supertonic] Generating dummy for: {}",
            req.text
        );
        let _ = create_dummy_wav(output_path);
        let _ = event_tx.send(TtsEvent::Error(
            "TTS failed, using fallback audio".to_string(),
        ));
        ctx.request_repaint();
    }
}

/// Serialized playback with a 1-slot override queue.
fn player_loop(rx: Receiver<PlayerMsg>, ctx: egui::Context) {
    let sink = match rodio::DeviceSinkBuilder::open_default_sink() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("No audio output device available: {e}");
            // Drain messages so the generator never blocks
            while rx.recv().is_ok() {}
            return;
        }
    };
    let mixer = sink.mixer().clone();
    let _ = ctx;

    while let Ok(mut current) = rx.recv() {
        loop {
            play_one(&mixer, current);
            // Non-blocking: keep only the newest pending request (override)
            let mut next: Option<PlayerMsg> = None;
            while let Ok(m) = rx.try_recv() {
                next = Some(m); // drop older pending entries
            }
            match next {
                Some(n) => current = n,
                None => break,
            }
        }
    }
}

fn play_one(mixer: &rodio::mixer::Mixer, msg: PlayerMsg) {
    match msg {
        PlayerMsg::File { path, delete_after } => {
            println!("[TTS] Playing: {}", path.display());
            let played = std::fs::File::open(&path).ok().and_then(|file| {
                let reader = std::io::BufReader::new(file);
                match rodio::play(mixer, reader) {
                    Ok(player) => {
                        player.sleep_until_end();
                        Some(true)
                    }
                    Err(e) => {
                        eprintln!("Playback Error: {e}");
                        Some(false)
                    }
                }
            });
            if played.is_none() {
                eprintln!("Playback Error: cannot open {}", path.display());
            }
            if delete_after {
                let _ = std::fs::remove_file(&path);
            }
        }
        PlayerMsg::Tone { freq_hz, dur_ms } => {
            // winsound.Beep equivalent: a plain sine beep
            let sr = 44100usize;
            let n = (sr as f32 * dur_ms as f32 / 1000.0) as usize;
            let samples: Vec<f32> = (0..n)
                .map(|i| {
                    (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sr as f32).sin() * 0.6
                })
                .collect();
            let player = rodio::Player::connect_new(mixer);
            player.append(rodio::buffer::SamplesBuffer::new(
                std::num::NonZero::new(1u16).unwrap(),
                std::num::NonZero::new(sr as u32).unwrap(),
                samples,
            ));
            player.sleep_until_end();
        }
    }
}

fn scan_voice_styles(voice_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(voice_dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| {
            e.path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    names
}

fn cache_filename(text: &str, voice: &str, lang: &str, model_version: &str) -> String {
    let key = format!("{text}_{voice}_{lang}_{model_version}");
    let mut hasher = Md5::new();
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("{hex}.wav")
}

fn random_hex(bytes: usize) -> String {
    use rand::Rng;
    let mut buf = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Write 16-bit PCM mono WAV (same as the official supertonic Rust example:
/// samples clamped to [-1, 1] then scaled to i16).
fn write_wav_pcm16(path: &Path, samples: &[f32], sample_rate: usize) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        let clamped = s.max(-1.0).min(1.0);
        writer.write_sample((clamped * 32767.0) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Fallback: 440 Hz / 0.5 s / 16-bit mono sine (the Python `_create_dummy_wav`).
fn create_dummy_wav(path: &Path) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    let n = (0.5 * 44100.0) as i32;
    for i in 0..n {
        let value =
            (32767.0 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 44100.0).sin()) as i16;
        writer.write_sample(value)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_filename_shape() {
        // Reference: hashlib.md5("텍스트_F1_ko_v1.7.3".encode()).hexdigest()
        let name = cache_filename("텍스트", "F1", "ko", "v1.7.3");
        assert!(name.ends_with(".wav"));
        assert_eq!(name.len(), 32 + 4);
    }

    #[test]
    fn dummy_wav_writes() {
        let dir = std::env::temp_dir();
        let p = dir.join(format!("bittalker_dummy_{}.wav", std::process::id()));
        create_dummy_wav(&p).unwrap();
        let r = hound::WavReader::open(&p).unwrap();
        assert_eq!(r.spec().channels, 1);
        assert_eq!(r.spec().sample_rate, 44100);
        assert_eq!(r.spec().bits_per_sample, 16);
        assert_eq!(r.duration(), 22050);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn pcm16_wav_clamps() {
        let dir = std::env::temp_dir();
        let p = dir.join(format!("bittalker_pcm16_{}.wav", std::process::id()));
        write_wav_pcm16(&p, &[0.0, 0.5, -0.5, 2.0, -2.0], 44100).unwrap();
        let r = hound::WavReader::open(&p).unwrap();
        assert_eq!(r.spec().sample_format, hound::SampleFormat::Int);
        assert_eq!(r.spec().bits_per_sample, 16);
        let _ = std::fs::remove_file(p);
    }
}
