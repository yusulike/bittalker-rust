//! Supertonic-3 ONNX TTS engine.
//!
//! Ported from two references, kept semantically aligned:
//! - `src/services/helper.py` of the original Python bittalker app
//!   (session tuning, config keys, service-facing API)
//! - the official Rust example of supertonic-3
//!   <https://github.com/supertone-oss-archive/supertonic/tree/main/rust>
//!   (ort value/extraction pattern, per-chunk WAV truncation to the
//!   predicted duration, 16-bit PCM output)
//!
//! Inference pipeline per text chunk (batch size 1):
//!   1. preprocess + tokenize (`UnicodeProcessor`)
//!   2. duration predictor  -> chunk duration (seconds)
//!   3. text encoder        -> text embedding [1, 256, L]
//!   4. sample noisy latent [1, 144, T] from `duration * sample_rate`
//!   5. `total_step` flow-matching steps of the vector estimator
//!   6. vocoder             -> waveform [1, samples]
//!   7. truncate to `duration * sample_rate` samples (drops vocoder padding)

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use ndarray::Array3;
use ort::session::{builder::GraphOptimizationLevel, Session};
use rand_distr::StandardNormal;
use rand::RngExt;

use super::processor::{chunk_text, length_to_mask_2d, UnicodeProcessor};

/// A voice style: cached tensors fed to the duration predictor and the
/// text-to-latent path.
#[derive(Clone)]
pub struct Style {
    /// shape [1, 50, 256]
    pub ttl: Array3<f32>,
    /// shape [1, 8, 16]
    pub dp: Array3<f32>,
}

/// Load one or more voice style JSON files as a batched style tensor.
pub fn load_voice_style(voice_style_paths: &[&Path]) -> Result<Style> {
    let first_path = voice_style_paths
        .first()
        .ok_or_else(|| anyhow!("no voice style paths given"))?;
    let first = read_style_file(first_path)?;

    let bsz = voice_style_paths.len();
    let (ttl_d1, ttl_d2) = first.ttl_dims;
    let (dp_d1, dp_d2) = first.dp_dims;

    let mut ttl_flat = vec![0f32; bsz * ttl_d1 * ttl_d2];
    let mut dp_flat = vec![0f32; bsz * dp_d1 * dp_d2];

    for (i, path) in voice_style_paths.iter().enumerate() {
        let data = if path == first_path {
            first.clone()
        } else {
            read_style_file(path)?
        };
        let ttl_offset = i * ttl_d1 * ttl_d2;
        for (idx, v) in data.ttl_data.into_iter().enumerate() {
            ttl_flat[ttl_offset + idx] = v;
        }
        let dp_offset = i * dp_d1 * dp_d2;
        for (idx, v) in data.dp_data.into_iter().enumerate() {
            dp_flat[dp_offset + idx] = v;
        }
    }

    let ttl_style = Array3::from_shape_vec((bsz, ttl_d1, ttl_d2), ttl_flat)?;
    let dp_style = Array3::from_shape_vec((bsz, dp_d1, dp_d2), dp_flat)?;

    Ok(Style {
        ttl: ttl_style,
        dp: dp_style,
    })
}

#[derive(Clone)]
struct RawStyle {
    ttl_dims: (usize, usize),
    dp_dims: (usize, usize),
    ttl_data: Vec<f32>,
    dp_data: Vec<f32>,
}

fn read_style_file(path: &Path) -> Result<RawStyle> {
    let root: serde_json::Value = serde_json::from_reader(std::fs::File::open(path)?)
        .with_context(|| format!("reading voice style {}", path.display()))?;

    let ttl = &root["style_ttl"];
    let dp = &root["style_dp"];
    let ttl_dims = dims_of(&ttl["dims"])?;
    let dp_dims = dims_of(&dp["dims"])?;
    let ttl_data = flatten_floats(&ttl["data"]);
    let dp_data = flatten_floats(&dp["data"]);

    // sanity: flattened data must fill the declared shape
    if ttl_data.len() != ttl_dims.0 * ttl_dims.1 {
        return Err(anyhow!(
            "style_ttl data length {} does not match dims {:?}",
            ttl_data.len(),
            ttl_dims
        ));
    }
    if dp_data.len() != dp_dims.0 * dp_dims.1 {
        return Err(anyhow!(
            "style_dp data length {} does not match dims {:?}",
            dp_data.len(),
            dp_dims
        ));
    }

    Ok(RawStyle {
        ttl_dims,
        dp_dims,
        ttl_data,
        dp_data,
    })
}

fn dims_of(v: &serde_json::Value) -> Result<(usize, usize)> {
    // dims is [1, d1, d2]; batch must be 1
    let arr = v.as_array().ok_or_else(|| anyhow!("style dims missing"))?;
    if arr.len() != 3 || arr[0].as_i64() != Some(1) {
        return Err(anyhow!("unexpected style dims: {v}"));
    }
    Ok((
        arr[1].as_u64().unwrap_or_default() as usize,
        arr[2].as_u64().unwrap_or_default() as usize,
    ))
}

/// Flatten a possibly-nested JSON number array (np.array(data).flatten()).
fn flatten_floats(v: &serde_json::Value) -> Vec<f32> {
    let mut out = Vec::new();
    fn walk(v: &serde_json::Value, out: &mut Vec<f32>) {
        match v {
            serde_json::Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    out.push(f as f32);
                }
            }
            _ => {}
        }
    }
    walk(v, &mut out);
    out
}

pub struct SupertonicEngine {
    dp_ort: Session,
    text_enc_ort: Session,
    vector_est_ort: Session,
    vocoder_ort: Session,
    processor: UnicodeProcessor,
    sample_rate: usize,
    base_chunk_size: usize,
    chunk_compress_factor: usize,
    ldim: usize,
    model_version: String,
}

impl SupertonicEngine {
    /// Load all four ONNX models plus configs from `onnx_dir`.
    pub fn load(onnx_dir: &Path) -> Result<Self> {
        let cfg_path = onnx_dir.join("tts.json");
        let cfgs: serde_json::Value =
            serde_json::from_reader(std::fs::File::open(&cfg_path)?)
                .with_context(|| format!("reading {}", cfg_path.display()))?;

        let sample_rate = cfgs["ae"]["sample_rate"].as_u64().unwrap_or(44100) as usize;
        let base_chunk_size = cfgs["ae"]["base_chunk_size"].as_u64().unwrap_or(512) as usize;
        let chunk_compress_factor =
            cfgs["ttl"]["chunk_compress_factor"].as_u64().unwrap_or(6) as usize;
        let ldim = cfgs["ttl"]["latent_dim"].as_u64().unwrap_or(24) as usize;
        let model_version = cfgs["tts_version"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        // Same CPU session tuning as the Python app (4 intra / 2 inter
        // threads, full graph optimization). The official Rust example uses
        // defaults; this is a strictly faster local tuning.
        let env = if let Some(path) = crate::paths::onnxruntime_dylib_path() {
            println!("Loading ONNX Runtime from: {}", path.display());
            ort::init_from(path).map_err(|e| anyhow!("{e}"))?.build()?
        } else {
            ort::init().build()?
        };
        let make_session = |file: &str| -> Result<Session> {
            let path = onnx_dir.join(file);
            Session::builder(&env)?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| anyhow!("{e}"))?
                .with_intra_threads(4)
                .map_err(|e| anyhow!("{e}"))?
                .with_inter_threads(2)
                .map_err(|e| anyhow!("{e}"))?
                .commit_from_file(&path)
                .with_context(|| format!("loading {}", path.display()))
        };

        let dp_ort = make_session("duration_predictor.onnx")?;
        let text_enc_ort = make_session("text_encoder.onnx")?;
        let vector_est_ort = make_session("vector_estimator.onnx")?;
        let vocoder_ort = make_session("vocoder.onnx")?;

        let processor = UnicodeProcessor::load(&onnx_dir.join("unicode_indexer.json"))?;

        println!(
            "Supertonic TTS initialized (version {model_version}, sample rate {sample_rate})"
        );

        Ok(Self {
            dp_ort,
            text_enc_ort,
            vector_est_ort,
            vocoder_ort,
            processor,
            sample_rate,
            base_chunk_size,
            chunk_compress_factor,
            ldim,
            model_version,
        })
    }

    pub fn model_version(&self) -> &str {
        &self.model_version
    }

    pub fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    /// Synthesize speech for a single utterance: chunks the text (120 chars
    /// for ko/ja, 300 otherwise), synthesizes each chunk and concatenates
    /// with `silence_duration` of silence between chunks. Each chunk's audio
    /// is truncated to the predicted duration (drops vocoder padding, same
    /// as the official Rust example).
    ///
    /// Returns `(mono f32 samples, total duration seconds)`.
    pub fn synthesize(
        &mut self,
        text: &str,
        lang: &str,
        style: &Style,
        total_step: usize,
        speed: f32,
        silence_duration: f32,
    ) -> Result<(Vec<f32>, f32)> {
        let max_len = if lang == "ko" || lang == "ja" { 120 } else { 300 };
        let text_list = chunk_text(text, max_len);
        if text_list.is_empty() {
            return Err(anyhow!("empty text"));
        }

        let mut wav_cat: Vec<f32> = Vec::new();
        let mut dur_cat: f32 = 0.0;
        let silence_len = (silence_duration * self.sample_rate as f32) as usize;

        for (i, chunk) in text_list.iter().enumerate() {
            let (wav, dur) = self.infer_single(chunk, lang, style, total_step, speed)?;

            // Truncate to the predicted speech duration
            let wav_len = (self.sample_rate as f32 * dur) as usize;
            let wav_chunk = &wav[..wav_len.min(wav.len())];

            if i == 0 {
                wav_cat.extend_from_slice(wav_chunk);
                dur_cat = dur;
            } else {
                wav_cat.extend(std::iter::repeat_n(0.0, silence_len));
                wav_cat.extend_from_slice(wav_chunk);
                dur_cat += silence_duration + dur;
            }
        }
        Ok((wav_cat, dur_cat))
    }

    /// One inference pass for a single text (batch size 1). Returns the full
    /// vocoder output plus the speed-adjusted duration.
    fn infer_single(
        &mut self,
        text: &str,
        lang: &str,
        style: &Style,
        total_step: usize,
        speed: f32,
    ) -> Result<(Vec<f32>, f32)> {
        let (text_ids, text_mask) = self.processor.encode(text, lang)?;

        let text_ids_value = ort::value::Value::from_array(text_ids)?;
        let text_mask_value = ort::value::Value::from_array(text_mask.clone())?;
        let style_dp_value = ort::value::Value::from_array(style.dp.clone())?;
        let style_ttl_value = ort::value::Value::from_array(style.ttl.clone())?;

        // --- duration predictor ---
        let mut duration: Vec<f32> = {
            let dp_outputs = self.dp_ort.run(ort::inputs! {
                "text_ids" => &text_ids_value,
                "style_dp" => &style_dp_value,
                "text_mask" => &text_mask_value,
            })?;
            let (_, duration_data) = dp_outputs["duration"].try_extract_tensor::<f32>()?;
            duration_data.to_vec()
        };

        // Apply speed factor to duration
        for d in duration.iter_mut() {
            *d /= speed;
        }

        // --- text encoder ---
        let text_emb = {
            let text_enc_outputs = self.text_enc_ort.run(ort::inputs! {
                "text_ids" => &text_ids_value,
                "style_ttl" => &style_ttl_value,
                "text_mask" => &text_mask_value,
            })?;
            let (text_emb_shape, text_emb_data) =
                text_enc_outputs["text_emb"].try_extract_tensor::<f32>()?;
            Array3::from_shape_vec(
                (
                    text_emb_shape[0] as usize,
                    text_emb_shape[1] as usize,
                    text_emb_shape[2] as usize,
                ),
                text_emb_data.to_vec(),
            )?
        };

        // --- noisy latent sampling ---
        let (mut xt, latent_mask) = self.sample_noisy_latent(&duration);

        // --- flow-matching denoising loop ---
        for step in 0..total_step {
            let xt_value = ort::value::Value::from_array(xt.clone())?;
            let text_emb_value = ort::value::Value::from_array(text_emb.clone())?;
            let style_ttl_value = ort::value::Value::from_array(style.ttl.clone())?;
            let latent_mask_value = ort::value::Value::from_array(latent_mask.clone())?;
            let text_mask_value = ort::value::Value::from_array(text_mask.clone())?;
            let current_step_value =
                ort::value::Value::from_array(ndarray::Array1::from_elem(1, step as f32))?;
            let total_step_value =
                ort::value::Value::from_array(ndarray::Array1::from_elem(1, total_step as f32))?;

            let (denoised_shape, denoised_data) = {
                let vector_est_outputs = self.vector_est_ort.run(ort::inputs! {
                    "noisy_latent" => &xt_value,
                    "text_emb" => &text_emb_value,
                    "style_ttl" => &style_ttl_value,
                    "latent_mask" => &latent_mask_value,
                    "text_mask" => &text_mask_value,
                    "current_step" => &current_step_value,
                    "total_step" => &total_step_value,
                })?;
                let (shape, data) =
                    vector_est_outputs["denoised_latent"].try_extract_tensor::<f32>()?;
                (shape.to_vec(), data.to_vec())
            };
            xt = Array3::from_shape_vec(
                (
                    denoised_shape[0] as usize,
                    denoised_shape[1] as usize,
                    denoised_shape[2] as usize,
                ),
                denoised_data,
            )?;
        }

        // --- vocoder ---
        let final_latent_value = ort::value::Value::from_array(xt)?;
        let wav: Vec<f32> = {
            let vocoder_outputs = self.vocoder_ort.run(ort::inputs! {
                "latent" => &final_latent_value,
            })?;
            let (_, wav_data) = vocoder_outputs["wav_tts"].try_extract_tensor::<f32>()?;
            wav_data.to_vec()
        };

        Ok((wav, duration[0]))
    }

    /// Sample the initial noisy latent for the given per-item durations
    /// (batch size 1, like the original `sample_noisy_latent`).
    fn sample_noisy_latent(&self, duration: &[f32]) -> (Array3<f32>, Array3<f32>) {
        let bsz = duration.len();
        let chunk_size = self.base_chunk_size * self.chunk_compress_factor;
        let latent_dim = self.ldim * self.chunk_compress_factor;

        // f32 math, matching the official Rust reference
        let max_dur = duration.iter().copied().fold(0.0f32, f32::max);
        let wav_len_max = (max_dur * self.sample_rate as f32) as usize;
        let wav_lengths: Vec<usize> = duration
            .iter()
            .map(|d| (*d * self.sample_rate as f32) as usize)
            .collect();
        let latent_len = (wav_len_max + chunk_size - 1) / chunk_size;

        // standard normal noise
        let mut noisy = Array3::<f32>::zeros((bsz, latent_dim, latent_len));
        let mut rng = rand::rng();
        for v in noisy.iter_mut() {
            *v = rng.sample::<f32, _>(StandardNormal);
        }

        // latent mask: ceil(wav_len / chunk) per item -> [B, 1, latent_len]
        let latent_lengths: Vec<usize> = wav_lengths
            .iter()
            .map(|&w| (w + chunk_size - 1) / chunk_size)
            .collect();
        let latent_mask = length_to_mask_2d(&latent_lengths, latent_len);

        // apply the latent mask
        for b in 0..bsz {
            for d in 0..latent_dim {
                for t in 0..latent_len {
                    noisy[[b, d, t]] *= latent_mask[[b, 0, t]];
                }
            }
        }
        (noisy, latent_mask)
    }
}

/// Convenience: load a single voice style by name from a voice-styles dir.
pub fn load_voice_style_by_name(dir: &Path, name: &str) -> Result<Style> {
    let path = dir.join(format!("{name}.json"));
    load_voice_style(&[&path])
}
