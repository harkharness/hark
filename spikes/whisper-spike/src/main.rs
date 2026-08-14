//! Spike 2: validate whisper-rs (metal) builds on Apple Silicon and measure
//! pt-BR transcription quality/latency on 16kHz mono wav files.
//!
//! Usage: whisper-spike <model.bin> <audio.wav>...

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn read_wav_mono_f32(path: &str) -> anyhow::Result<Vec<f32>> {
    let reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    anyhow::ensure!(
        spec.sample_rate == 16_000 && spec.channels == 1,
        "expected 16kHz mono, got {}Hz {}ch",
        spec.sample_rate,
        spec.channels
    );
    Ok(reader
        .into_samples::<i16>()
        .map(|s| s.map(|v| v as f32 / 32768.0))
        .collect::<Result<_, _>>()?)
}

fn transcribe(ctx: &WhisperContext, samples: &[f32]) -> anyhow::Result<String> {
    let mut state = ctx.create_state()?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("pt"));
    // Bias decoding toward the tech vocabulary we expect in commands.
    params.set_initial_prompt(
        "Comandos sobre desenvolvimento: webhook, pull request, PR, deploy, \
         Claude Code, branch, commit, migração, cluster, vox.",
    );
    params.set_print_progress(false);
    params.set_print_special(false);
    params.set_print_realtime(false);
    state.full(params, samples)?;
    let n = state.full_n_segments()?;
    Ok((0..n)
        .map(|i| state.full_get_segment_text(i))
        .collect::<Result<Vec<_>, _>>()?
        .join(""))
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let model = args.next().expect("usage: whisper-spike <model.bin> <wav>...");
    let load0 = std::time::Instant::now();
    let ctx = WhisperContext::new_with_params(&model, WhisperContextParameters::default())?;
    eprintln!("model loaded in {:?}", load0.elapsed());

    for wav in args {
        let samples = read_wav_mono_f32(&wav)?;
        let t0 = std::time::Instant::now();
        let text = transcribe(&ctx, &samples)?;
        println!("{wav}\n  [{:?}] {}", t0.elapsed(), text.trim());
    }
    Ok(())
}
