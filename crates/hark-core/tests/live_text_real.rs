//! The live text against the real speech model, on a recording made by
//! the system's own voice. Ignored by default: it needs the model on disk
//! and a WAV (16 kHz mono, 16-bit) named by HARK_LIVE_WAV.
//!
//!   say -o /tmp/fala.wav --data-format=LEI16@16000 "…"
//!   HARK_LIVE_WAV=/tmp/fala.wav HARK_LIVE_MODEL=~/.hark/models/ggml-small.bin \
//!     cargo test -p hark-core --test live_text_real -- --ignored --nocapture

use hark_core::adapters::whisper_stt::WhisperStt;
use hark_core::app::live_text::LiveText;
use hark_core::domain::vad::VadConfig;
use hark_core::ports::Stt;

fn wav_samples(path: &str) -> Vec<f32> {
    let bytes = std::fs::read(path).expect("the wav");
    // The data chunk, wherever the header put it.
    let at = bytes.windows(4).position(|w| w == b"data").expect("a data chunk") + 8;
    bytes[at..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b) as f32 / 32768.0)
        .collect()
}

#[test]
#[ignore]
fn the_words_appear_while_the_recording_plays() {
    let wav = std::env::var("HARK_LIVE_WAV").expect("HARK_LIVE_WAV");
    let model = std::env::var("HARK_LIVE_MODEL").expect("HARK_LIVE_MODEL");
    let samples = wav_samples(&wav);
    let stt = WhisperStt::load(std::path::Path::new(&model), "pt", &[]).expect("the model");
    let cfg = VadConfig::default();
    let mut live = LiveText::default();
    let mut read = |audio: &[f32]| stt.transcribe(audio).map(Some);

    let step = 16_000 / 2; // the capture as it would be seen every half second
    let mut reads = 0usize;
    let started = std::time::Instant::now();
    let mut at = step;
    while at < samples.len() {
        let t0 = std::time::Instant::now();
        if let Some(text) = live.pass(&samples[..at], &cfg, &mut read).expect("a pass") {
            reads += 1;
            println!("{:>5.1}s  ({:>4}ms)  {text}", at as f32 / 16_000.0, t0.elapsed().as_millis());
        }
        at += step;
    }
    let t0 = std::time::Instant::now();
    let said = live.finish(&samples, &cfg, &mut read).expect("the end").expect("not cut");
    println!("final  ({:>4}ms)  {said}", t0.elapsed().as_millis());
    println!("whole recording in one read: {}", stt.transcribe(&samples).expect("whole"));
    println!("{reads} live updates over {:.1}s of audio, {:?} of compute", samples.len() as f32 / 16_000.0, started.elapsed());
    assert!(reads >= 3, "the text moved while the recording played");
    assert!(said.to_lowercase().contains("pendente"));
    assert!(said.to_lowercase().contains("board") || said.to_lowercase().contains("infra"));
}
