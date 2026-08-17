//! Microphone capture via cpal: resampled to 16kHz mono f32, cut by the
//! pure VAD when the utterance ends.

use crate::domain::vad::{end_of_speech, VadConfig};
use crate::ports::AudioIn;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

pub struct CpalMic {
    pub vad: VadConfig,
    /// Hard cap so a noisy room can't record forever.
    pub max_seconds: usize,
    /// Manual cut (Esc in the window): set true and the capture returns
    /// whatever was said so far, without waiting for the VAD.
    pub stop: Arc<AtomicBool>,
}

impl Default for CpalMic {
    fn default() -> Self {
        Self {
            vad: VadConfig::default(),
            max_seconds: 30,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AudioIn for CpalMic {
    fn record_utterance(&self) -> anyhow::Result<Vec<f32>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("no input device (check macOS mic permission)"))?;
        let config = device.default_input_config()?;
        let src_rate = config.sample_rate().0 as usize;
        let channels = config.channels() as usize;

        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                // Downmix to mono on the audio thread; keep it cheap.
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                    .collect();
                let _ = tx.send(mono);
            },
            |err| eprintln!("audio input error: {err}"),
            None,
        )?;
        stream.play()?;

        let mut raw: Vec<f32> = Vec::new();
        let max_raw = src_rate * self.max_seconds;
        loop {
            let Ok(chunk) = rx.recv_timeout(std::time::Duration::from_secs(2)) else {
                anyhow::bail!("microphone produced no audio (permission denied?)");
            };
            raw.extend(chunk);
            let resampled = resample_to_16k(&raw, src_rate);
            if self.stop.swap(false, Ordering::SeqCst) {
                drop(stream);
                return Ok(resampled);
            }
            if let Some(end) = end_of_speech(&resampled, &self.vad) {
                drop(stream);
                return Ok(resampled[..end].to_vec());
            }
            if raw.len() >= max_raw {
                drop(stream);
                return Ok(resampled);
            }
        }
    }
}

/// Naive decimation/linear resample; speech quality is fine for STT.
fn resample_to_16k(samples: &[f32], src_rate: usize) -> Vec<f32> {
    if src_rate == 16_000 {
        return samples.to_vec();
    }
    let ratio = src_rate as f32 / 16_000.0;
    let out_len = (samples.len() as f32 / ratio) as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f32 * ratio;
            let base = pos as usize;
            let frac = pos - base as f32;
            let a = samples.get(base).copied().unwrap_or(0.0);
            let b = samples.get(base + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}
