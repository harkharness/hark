//! whisper.cpp speech-to-text — Metal on Apple Silicon, CPU everywhere else
//! (see hark-core's Cargo.toml). Model loads once; a silent warmup at startup
//! absorbs the ~10s Metal shader compilation so the first real utterance stays
//! fast. On CPU there are no shaders to compile, but an utterance still costs
//! a full 30s encoder pass whatever its length — which is why the transcription
//! is abortable there (see spikes/FINDINGS.md, spikes 2 and 4).

use crate::ports::Stt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
            "whisper model not found at {} (run: hark setup)",
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

    /// Like `transcribe`, but gives up mid-compute when `abort` flips true.
    /// On CPU (Intel: no Metal) one utterance costs tens of seconds — Esc
    /// cannot wait for whisper to finish just to throw the text away.
    pub fn transcribe_with_abort(
        &self,
        samples: &[f32],
        abort: Arc<AtomicBool>,
    ) -> anyhow::Result<Option<String>> {
        let flag = abort.clone();
        let out = self.run(samples, Some(Box::new(move || flag.load(Ordering::SeqCst))));
        // Aborted wins over the outcome: whether whisper bailed out (Err)
        // or finished right as Esc landed (Ok), the turn is dead and no
        // text may come back.
        match abort.load(Ordering::SeqCst) {
            true => Ok(None),
            false => out.map(Some),
        }
    }

    fn run(
        &self,
        samples: &[f32],
        abort: Option<Box<dyn FnMut() -> bool + 'static>>,
    ) -> anyhow::Result<String> {
        let mut state = self.ctx.create_state()?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.language));
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        if !self.vocab_bias.is_empty() {
            params.set_initial_prompt(&self.vocab_bias);
        }
        if let Some(cb) = abort {
            params.set_abort_callback_safe(cb);
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

impl Stt for WhisperStt {
    fn transcribe(&self, samples: &[f32]) -> anyhow::Result<String> {
        self.run(samples, None)
    }
}
