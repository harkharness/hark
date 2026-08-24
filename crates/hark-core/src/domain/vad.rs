//! Pure voice-activity detection over 16kHz mono f32 samples.
//! Strategy: RMS per window; speech starts when RMS crosses the threshold,
//! ends after `hangover` consecutive silent windows.

/// All tunables in one place (config-overridable later).
#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    pub sample_rate: usize,
    /// Window length in milliseconds.
    pub window_ms: usize,
    /// RMS above this counts as speech.
    pub threshold: f32,
    /// Silence duration (ms) that closes an utterance.
    pub hangover_ms: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000,
            window_ms: 30,
            threshold: 0.015,
            // 900ms cut people off mid-sentence: a pause to think while
            // dictating an instruction is longer than that, and losing
            // half a sentence costs a whole turn. Dictation is not a
            // wake-word — waiting a beat longer is cheaper than a retry.
            hangover_ms: 1_700,
        }
    }
}

/// Given everything recorded so far, decide whether the utterance is over.
/// Returns the sample index where speech (plus hangover) ends.
/// None while still silent at the start or still talking.
pub fn end_of_speech(samples: &[f32], cfg: &VadConfig) -> Option<usize> {
    let window = cfg.sample_rate * cfg.window_ms / 1000;
    if window == 0 {
        return None;
    }
    let needed_silent = cfg.hangover_ms.div_ceil(cfg.window_ms);

    let mut spoke = false;
    let mut silent_run = 0usize;
    for (i, chunk) in samples.chunks(window).enumerate() {
        if chunk.len() < window {
            break; // incomplete tail window: wait for more audio
        }
        if rms(chunk) >= cfg.threshold {
            spoke = true;
            silent_run = 0;
        } else if spoke {
            silent_run += 1;
            if silent_run >= needed_silent {
                return Some((i + 1) * window);
            }
        }
    }
    None
}

fn rms(chunk: &[f32]) -> f32 {
    (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests describe the ALGORITHM, so they pin their own hangover
    /// instead of riding on the shipped default — tuning the product
    /// should not make the maths look broken.
    fn cfg() -> VadConfig {
        VadConfig {
            hangover_ms: 900,
            ..VadConfig::default()
        }
    }

    #[test]
    fn the_shipped_hangover_leaves_room_to_think() {
        // 900ms cut people off mid-sentence while they were dictating an
        // instruction. The product default is a separate decision from
        // the algorithm, and it is deliberately generous.
        assert!(
            VadConfig::default().hangover_ms >= 1_500,
            "a dictation pause is longer than a wake-word pause"
        );
    }

    fn silence(ms: usize) -> Vec<f32> {
        vec![0.0; 16 * ms]
    }

    fn speech(ms: usize) -> Vec<f32> {
        // Loud alternating signal, RMS ~0.5.
        (0..16 * ms).map(|i| if i % 2 == 0 { 0.5 } else { -0.5 }).collect()
    }

    #[test]
    fn silence_alone_never_ends() {
        assert_eq!(end_of_speech(&silence(3000), &cfg()), None);
    }

    #[test]
    fn ongoing_speech_does_not_end() {
        let mut audio = silence(200);
        audio.extend(speech(1000));
        assert_eq!(end_of_speech(&audio, &cfg()), None);
    }

    #[test]
    fn speech_then_hangover_of_silence_ends() {
        let mut audio = silence(200);
        audio.extend(speech(1000));
        audio.extend(silence(1200));
        let end = end_of_speech(&audio, &cfg()).expect("utterance should end");
        // Ends somewhere inside the trailing silence, after the speech.
        assert!(end > 16 * 1200);
        assert!(end <= audio.len());
    }

    #[test]
    fn short_pause_inside_speech_does_not_end() {
        let mut audio = speech(600);
        audio.extend(silence(400)); // below 900ms hangover
        audio.extend(speech(600));
        assert_eq!(end_of_speech(&audio, &cfg()), None);
    }
}
