//! Diagnostic: measure model load / warmup / transcription wall time on
//! THIS machine. Usage: stt_bench <model.bin> <audio.wav 16k mono i16> <lang>

use hark_core::adapters::whisper_stt::WhisperStt;
use hark_core::ports::Stt;
use std::time::Instant;

fn read_wav_16k_mono_i16(path: &str) -> Vec<f32> {
    let bytes = std::fs::read(path).expect("read wav");
    // Minimal RIFF walk: find the "data" chunk, assume LEI16 payload.
    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if id == b"data" {
            let data = &bytes[pos + 8..pos + 8 + size.min(bytes.len() - pos - 8)];
            return data
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                .collect();
        }
        pos += 8 + size + (size & 1);
    }
    panic!("no data chunk in {path}");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let model = args.next().expect("model path");
    let wav = args.next().expect("wav path");
    let lang = args.next().unwrap_or_else(|| "pt".into());
    // 4th arg: the initial-prompt bias, to price what speech_bias costs.
    let bias: Vec<String> = args.next().into_iter().collect();

    let samples = read_wav_16k_mono_i16(&wav);
    let audio_secs = samples.len() as f64 / 16_000.0;
    println!("audio: {:.1}s ({} samples)", audio_secs, samples.len());

    let t = Instant::now();
    let stt = WhisperStt::load(std::path::Path::new(&model), &lang, &bias).expect("load");
    println!("load: {:.1}s", t.elapsed().as_secs_f64());

    let t = Instant::now();
    stt.warmup();
    println!("warmup (1s silence): {:.1}s", t.elapsed().as_secs_f64());

    let t = Instant::now();
    let text = stt.transcribe(&samples).expect("transcribe");
    let secs = t.elapsed().as_secs_f64();
    println!(
        "transcribe: {:.1}s (RTF {:.1}x) -> \"{}\"",
        secs,
        secs / audio_secs,
        text
    );
}
