//! Microphone capture via cpal: resampled to 16kHz mono f32, cut by the
//! pure VAD when the utterance ends.

use crate::domain::vad::{end_of_speech, heard_voice, VadConfig};
use crate::ports::AudioIn;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

pub struct CpalMic {
    pub vad: VadConfig,
    /// Hard cap so a noisy room can't record forever. Long enough for a
    /// long dictation: 30s cut people off mid-thought (23/11).
    pub max_seconds: usize,
    /// How long to wait for a voice before giving up. None waits for the
    /// cap; a conversation that re-arms the microphone by itself sets it,
    /// so a room nobody is talking in ends the conversation.
    pub patience_secs: Option<usize>,
    /// Manual cut (Esc in the window): set true and the capture returns
    /// whatever was said so far, without waiting for the VAD.
    pub stop: Arc<AtomicBool>,
}

impl Default for CpalMic {
    fn default() -> Self {
        Self {
            vad: VadConfig::default(),
            max_seconds: 300,
            patience_secs: None,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Why a capture stopped: the log says it, so "it kept listening" and
/// "it cut me off" can be told apart from a line of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureEnd {
    /// The voice stopped and the room stayed quiet for the hangover.
    Silence,
    /// The person ended it (the microphone button, Enter, Esc).
    Stopped,
    /// The hard cap: nothing ever read as the end of the utterance.
    Cap,
    /// Nobody spoke within the patience given.
    NoVoice,
}

impl CaptureEnd {
    pub fn as_str(self) -> &'static str {
        match self {
            CaptureEnd::Silence => "silence",
            CaptureEnd::Stopped => "the person",
            CaptureEnd::Cap => "the cap",
            CaptureEnd::NoVoice => "nobody speaking",
        }
    }
}

/// One utterance: 16kHz mono, why it ended, and the level that told
/// voice from room in it.
pub struct Capture {
    pub samples: Vec<f32>,
    pub end: CaptureEnd,
    pub cut: f32,
}

impl AudioIn for CpalMic {
    fn record_utterance(&self) -> anyhow::Result<Vec<f32>> {
        Ok(self.record()?.samples)
    }
}

impl CpalMic {
    /// Record one utterance and say how it ended.
    pub fn record(&self) -> anyhow::Result<Capture> {
        let shared = Mutex::new(Vec::new());
        let end = self.record_into(&shared)?;
        let samples = shared.into_inner().unwrap_or_else(|e| e.into_inner());
        let cut = crate::domain::vad::threshold_for(&samples, &self.vad);
        Ok(Capture { samples, end, cut })
    }

    /// Record one utterance INTO a buffer someone else is reading: the
    /// audio lands there as it arrives (16kHz mono), so the words can be
    /// read while they are being said (`app::live_text`). Returns how the
    /// capture ended; the buffer holds the utterance.
    pub fn record_into(&self, shared: &Mutex<Vec<f32>>) -> anyhow::Result<CaptureEnd> {
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

        let cap = 16_000 * self.max_seconds;
        // Loudest sample seen. A denied microphone does NOT stop the
        // callbacks — CoreAudio keeps delivering buffers of zeros, so the
        // 2s timeout never fires and the VAD never sees speech to end.
        // Without this the capture just ran to the cap on nothing.
        let mut peak = 0.0f32;
        let mut last_check = std::time::Instant::now();
        loop {
            let Ok(chunk) = rx.recv_timeout(std::time::Duration::from_secs(2)) else {
                anyhow::bail!("microphone produced no audio (permission denied?)");
            };
            peak = chunk.iter().fold(peak, |m, s| m.max(s.abs()));
            // Each chunk is resampled on its own and appended: resampling
            // the whole buffer on every callback was O(n²), which a
            // five-minute dictation would have felt.
            let len = {
                let mut all = shared.lock().unwrap_or_else(|e| e.into_inner());
                all.extend(resample_to_16k(&chunk, src_rate));
                all.len()
            };

            // The manual cut is checked on EVERY chunk: Esc has to land
            // within a chunk, not within a VAD window.
            if self.stop.swap(false, Ordering::SeqCst) {
                drop(stream);
                return Ok(CaptureEnd::Stopped);
            }

            let full = len >= cap;
            // A VAD decision every 250ms is as good as one per chunk and
            // far cheaper.
            if !full && last_check.elapsed() < std::time::Duration::from_millis(250) {
                continue;
            }
            last_check = std::time::Instant::now();

            if peak < 1e-4 && len >= 16_000 * 3 {
                drop(stream);
                anyhow::bail!(
                    "microphone is delivering silence — check System Settings › Privacy & Security › Microphone"
                );
            }

            let mut all = shared.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(end) = end_of_speech(&all, &self.vad) {
                drop(stream);
                all.truncate(end);
                return Ok(CaptureEnd::Silence);
            }
            if self.patience_secs.is_some_and(|secs| len >= 16_000 * secs) && !heard_voice(&all, &self.vad) {
                drop(stream);
                return Ok(CaptureEnd::NoVoice);
            }
            if full {
                drop(stream);
                return Ok(CaptureEnd::Cap);
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
