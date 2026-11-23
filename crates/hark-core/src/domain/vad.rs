//! Pure voice-activity detection over 16kHz mono f32 samples.
//! Strategy: RMS per window; speech starts when RMS crosses the cut,
//! ends after `hangover` consecutive silent windows. The cut is the
//! configured threshold in a quiet room and rises over the room's own
//! level in a noisy one (`threshold_for`).

/// All tunables in one place (config-overridable later).
#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    pub sample_rate: usize,
    /// Window length in milliseconds.
    pub window_ms: usize,
    /// RMS above this counts as speech in a quiet room: the floor of the
    /// cut, which a noisy room raises.
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
            // 1.7s read as "it did not hear me stop" (23/11): the wait is
            // the shortest the dictation pause allows, and the microphone
            // button, Enter or Esc end the capture at once for whoever
            // will not wait even that.
            hangover_ms: 1_500,
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
    let levels = window_levels(samples, window);
    let cut = cut_over(&levels, cfg)?;

    let mut spoke = false;
    let mut silent_run = 0usize;
    for (i, level) in levels.iter().enumerate() {
        if *level >= cut {
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

/// The level that separates voice from room in this recording, for the
/// log: the configured threshold in a quiet room, higher in a noisy one.
pub fn threshold_for(samples: &[f32], cfg: &VadConfig) -> f32 {
    let window = cfg.sample_rate * cfg.window_ms / 1000;
    if window == 0 {
        return cfg.threshold;
    }
    cut_over(&window_levels(samples, window), cfg).unwrap_or(cfg.threshold)
}

/// RMS of every COMPLETE window (the tail waits for more audio).
fn window_levels(samples: &[f32], window: usize) -> Vec<f32> {
    samples.chunks(window).filter(|c| c.len() == window).map(rms).collect()
}

/// How far over the room a window must be to count as voice.
const OVER_ROOM: f32 = 3.0;
/// And never more than this share of the voice itself, so a room
/// measured while someone was already talking cannot outshout them.
const OF_VOICE: f32 = 0.5;

/// The cut for these windows, or None when nothing stands out of the
/// room: a fixed threshold called a fan "speech" and never heard it stop.
/// The room is the quiet fifth of the windows, the voice the loud end;
/// the cut is three times the room, at least the configured threshold
/// and at most half the voice.
fn cut_over(levels: &[f32], cfg: &VadConfig) -> Option<f32> {
    if levels.is_empty() {
        return None;
    }
    let mut sorted = levels.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let at = |share: f32| sorted[((sorted.len() - 1) as f32 * share) as usize];
    let room = at(0.2);
    let voice = at(0.95);
    if voice < cfg.threshold {
        return None;
    }
    let cut = cfg.threshold.max((room * OVER_ROOM).min(voice * OF_VOICE));
    // A level that never rises over its own floor is a room, not a voice.
    (voice >= cut && voice > room * 1.5).then_some(cut)
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

    fn noise(ms: usize, level: f32) -> Vec<f32> {
        (0..16 * ms).map(|i| if i % 2 == 0 { level } else { -level }).collect()
    }

    /// 23/11: with a fan on, or a microphone with automatic gain, the
    /// room's own level sits above the fixed threshold: "silence" was
    /// never silent, and the capture ran to its 30 s cap after the person
    /// had stopped talking. The room is measured and the cut set over it.
    #[test]
    fn a_noisy_room_still_ends_when_the_voice_stops() {
        let room = 0.03; // twice the fixed threshold
        let mut audio = noise(400, room);
        audio.extend(speech(1500));
        audio.extend(noise(2000, room));
        let end = end_of_speech(&audio, &cfg()).expect("the voice stopped: the utterance is over");
        assert!(end > 16 * 1900 && end <= audio.len());
        // And the cut that was used sits between the room and the voice.
        let cut = threshold_for(&audio, &cfg());
        assert!(cut > room && cut < 0.5, "cut {cut}");
    }

    /// A room that is only loud is not someone talking: with nothing
    /// standing out of the noise there is no utterance to end.
    #[test]
    fn a_loud_room_with_nobody_talking_never_ends() {
        assert_eq!(end_of_speech(&noise(5000, 0.03), &cfg()), None);
    }

    /// In a quiet room the cut is the configured one, as it always was.
    #[test]
    fn a_quiet_room_keeps_the_configured_cut() {
        let mut audio = silence(300);
        audio.extend(speech(1000));
        audio.extend(silence(1200));
        assert_eq!(threshold_for(&audio, &cfg()), cfg().threshold);
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
