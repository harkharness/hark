//! whisper.cpp (Metal) speech-to-text. Model loads once; a silent warmup at
//! startup absorbs the ~10s Metal shader compilation so the first real
//! utterance stays fast (see spikes/FINDINGS.md).

use crate::ports::Stt;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperStt {
    ctx: WhisperContext,
    language: String,
    vocab_bias: String,
}

impl WhisperStt {
    pub fn load(model_path: &std::path::Path, language: &str, vocab: &[String]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            model_path.exists(),
            "whisper model not found at {} (run: vox setup)",
            model_path.display()
        );
        let ctx = WhisperContext::new_with_params(
            &model_path.to_string_lossy(),
            WhisperContextParameters::default(),
        )?;
        Ok(Self {
            ctx,
            language: language.to_string(),
            vocab_bias: vocab.join(", "),
        })
    }

    /// One silent second through the model to trigger Metal warmup.
    pub fn warmup(&self) {
        let _ = self.transcribe(&vec![0.0f32; 16_000]);
    }
}

impl Stt for WhisperStt {
    fn transcribe(&self, samples: &[f32]) -> anyhow::Result<String> {
        let mut state = self.ctx.create_state()?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.language));
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        if !self.vocab_bias.is_empty() {
            params.set_initial_prompt(&self.vocab_bias);
        }
        state.full(params, samples)?;
        let n = state.full_n_segments()?;
        let text = (0..n)
            .map(|i| state.full_get_segment_text(i))
            .collect::<Result<Vec<_>, _>>()?
            .join("");
        Ok(text.trim().to_string())
    }
}
