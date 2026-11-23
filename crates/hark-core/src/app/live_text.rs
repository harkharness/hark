//! The words as they are said (fase 16, 23/11).
//!
//! The window showed a waveform until the capture ended and the whole
//! recording had been read: nobody could tell whether it was hearing
//! them, and a long dictation ended in a long wait. `LiveText` reads the
//! audio WHILE it is being recorded. What was said before a pause is
//! settled: read once, kept, never read again. What is being said now is
//! the open tail, read again on every pass until a pause settles it. So a
//! pass costs the length of the current phrase, not of the recording, and
//! when the capture ends only the last phrase is left to read.

use crate::domain::vad::{has_voice, phrase_breaks, threshold_for, VadConfig};

/// A pause this long settles what came before it.
pub const PHRASE_GAP_MS: usize = 600;
/// An open tail shorter than this is not worth a pass yet.
const MIN_TAIL_SAMPLES: usize = 16_000 * 7 / 10;

/// Reads a stretch of audio. `None` says the read was cut short (the
/// capture ended under it): nothing may be concluded from it.
pub type Reader<'a> = dyn FnMut(&[f32]) -> anyhow::Result<Option<String>> + 'a;

#[derive(Debug, Default)]
pub struct LiveText {
    /// What every settled phrase said, in order.
    settled: String,
    /// The sample the settled part ends at.
    upto: usize,
    /// The open tail's last reading.
    tail: String,
    /// How long the tail was when it was last read: a tail that did not
    /// grow is not read again.
    tail_read_at: usize,
}

impl LiveText {
    /// What to show right now: the settled phrases and the open one.
    pub fn text(&self) -> String {
        join(&self.settled, &self.tail)
    }

    /// One pass over everything recorded so far. Settles what a pause
    /// closed since the last pass, then reads the open tail. Returns the
    /// text to show, or None when nothing changed.
    pub fn pass(&mut self, samples: &[f32], cfg: &VadConfig, read: &mut Reader) -> anyhow::Result<Option<String>> {
        let before = self.text();
        let cut = threshold_for(samples, cfg);
        if let Some(&brk) = phrase_breaks(samples, cfg, PHRASE_GAP_MS).iter().rev().find(|b| **b > self.upto) {
            let brk = brk.min(samples.len());
            if has_voice(&samples[self.upto..brk], cfg, cut) {
                // Cut short: the phrase stays open and is read next time.
                let Some(said) = read(&samples[self.upto..brk])? else { return Ok(None) };
                self.settled = join(&self.settled, &said);
            }
            self.upto = brk;
            self.tail.clear();
            self.tail_read_at = 0;
        }
        let tail = &samples[self.upto..];
        if tail.len() >= MIN_TAIL_SAMPLES && tail.len() > self.tail_read_at && has_voice(tail, cfg, cut) {
            if let Some(said) = read(tail)? {
                self.tail = said;
                self.tail_read_at = tail.len();
            }
        }
        let now = self.text();
        Ok((now != before).then_some(now))
    }

    /// The capture is over: read whatever is still open and hand back the
    /// whole utterance. `None` when the read was cut short (Esc).
    pub fn finish(mut self, samples: &[f32], cfg: &VadConfig, read: &mut Reader) -> anyhow::Result<Option<String>> {
        let cut = threshold_for(samples, cfg);
        let upto = self.upto.min(samples.len());
        let tail = &samples[upto..];
        if has_voice(tail, cfg, cut) {
            // A tail already read to its end is not read twice.
            if tail.len() > self.tail_read_at {
                let Some(said) = read(tail)? else { return Ok(None) };
                self.tail = said;
            }
        } else {
            self.tail.clear();
        }
        Ok(Some(self.text()))
    }
}

fn join(a: &str, b: &str) -> String {
    match (a.trim(), b.trim()) {
        ("", b) => b.to_string(),
        (a, "") => a.to_string(),
        (a, b) => format!("{a} {b}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speech(ms: usize) -> Vec<f32> {
        (0..16 * ms).map(|i| if i % 2 == 0 { 0.5 } else { -0.5 }).collect()
    }
    fn silence(ms: usize) -> Vec<f32> {
        vec![0.0; 16 * ms]
    }
    fn cfg() -> VadConfig {
        VadConfig { hangover_ms: 1500, ..VadConfig::default() }
    }

    /// A reader that "hears" how long the stretch was, and counts its calls.
    fn reader(calls: &mut Vec<usize>) -> impl FnMut(&[f32]) -> anyhow::Result<Option<String>> + '_ {
        move |audio| {
            calls.push(audio.len());
            Ok(Some(format!("[{}ms]", audio.len() / 16)))
        }
    }

    #[test]
    fn the_words_show_while_they_are_said() {
        let mut calls = Vec::new();
        let mut live = LiveText::default();
        let audio = speech(1000);
        let shown = live.pass(&audio, &cfg(), &mut reader(&mut calls)).unwrap();
        assert_eq!(shown.as_deref(), Some("[1000ms]"));
        // Nothing new was said: nothing is read, nothing changes.
        assert_eq!(live.pass(&audio, &cfg(), &mut reader(&mut calls)).unwrap(), None);
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn what_a_pause_closed_is_read_once_and_kept() {
        let mut calls = Vec::new();
        let mut live = LiveText::default();
        let mut audio = speech(1000);
        audio.extend(silence(800));
        audio.extend(speech(1000));
        let first = live.pass(&audio, &cfg(), &mut reader(&mut calls)).unwrap().unwrap();
        // The first phrase settled (with half its pause), the second is open.
        assert!(first.starts_with("[1") && first.contains(' '), "{first}");
        let settled_calls = calls.len();

        audio.extend(speech(1000));
        let second = live.pass(&audio, &cfg(), &mut reader(&mut calls)).unwrap().unwrap();
        // Only the open phrase was read again: the settled one kept its words.
        assert_eq!(calls.len(), settled_calls + 1);
        assert_eq!(second.split(' ').next(), first.split(' ').next());
        assert!(*calls.last().unwrap() < audio.len(), "the pass read the tail, not the recording");
    }

    #[test]
    fn the_end_reads_only_what_is_still_open() {
        let mut calls = Vec::new();
        let mut live = LiveText::default();
        let mut audio = speech(1000);
        audio.extend(silence(800));
        audio.extend(speech(1000));
        live.pass(&audio, &cfg(), &mut reader(&mut calls)).unwrap();
        audio.extend(speech(300));
        audio.extend(silence(1500));
        let n = calls.len();
        let said = live.finish(&audio, &cfg(), &mut reader(&mut calls)).unwrap().unwrap();
        assert_eq!(calls.len(), n + 1);
        assert_eq!(said.split(' ').count(), 2, "{said}");
    }

    #[test]
    fn a_read_cut_short_concludes_nothing() {
        let mut live = LiveText::default();
        let mut audio = speech(1000);
        audio.extend(silence(800));
        audio.extend(speech(1000));
        let mut cut = |_: &[f32]| -> anyhow::Result<Option<String>> { Ok(None) };
        assert_eq!(live.pass(&audio, &cfg(), &mut cut).unwrap(), None);
        // The phrase is still open: a later pass reads it whole.
        let mut calls = Vec::new();
        let shown = live.pass(&audio, &cfg(), &mut reader(&mut calls)).unwrap().unwrap();
        assert_eq!(shown.split(' ').count(), 2);
        assert_eq!(live.finish(&audio, &cfg(), &mut cut).unwrap(), Some(shown));
    }

    #[test]
    fn a_recording_with_nobody_in_it_says_nothing() {
        let mut calls = Vec::new();
        let live = LiveText::default();
        let said = live.finish(&silence(3000), &cfg(), &mut reader(&mut calls)).unwrap();
        assert_eq!(said.as_deref(), Some(""));
        assert!(calls.is_empty(), "silence is never handed to the reader");
    }
}
