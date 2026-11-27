//! What a `!command` typed in the composer leaves behind.
//!
//! A command can print megabytes; a chat and an agent's context cannot
//! hold them. The tape keeps the start and the end of what was printed,
//! the two places an answer is usually read from, and counts what it let
//! go in between.

use std::collections::VecDeque;

/// The line that stands where output was dropped.
pub const GAP: &str = "\n…\n";

/// Output as it is printed, kept to a budget.
pub struct Tape {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    head_room: usize,
    tail_room: usize,
    omitted: usize,
}

/// What the tape kept, as text.
#[derive(Debug, PartialEq, Eq)]
pub struct Kept {
    pub text: String,
    /// Bytes dropped from the middle. Zero when all of it was kept.
    pub omitted: usize,
}

impl Tape {
    /// A tape that keeps at most `max` bytes: half from the start, half
    /// from the end.
    pub fn new(max: usize) -> Self {
        let tail_room = max / 2;
        Self {
            head: Vec::new(),
            tail: VecDeque::new(),
            head_room: max - tail_room,
            tail_room,
            omitted: 0,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        let free = self.head_room - self.head.len();
        let (first, rest) = bytes.split_at(free.min(bytes.len()));
        self.head.extend_from_slice(first);
        // Whatever no longer fits at the end falls off its front: the
        // tail is always the last thing printed.
        let over = (self.tail.len() + rest.len()).saturating_sub(self.tail_room);
        let from_tail = over.min(self.tail.len());
        self.tail.drain(..from_tail);
        self.tail.extend(&rest[over - from_tail..]);
        self.omitted += over;
    }

    pub fn finish(self) -> Kept {
        let mut head = self.head;
        let mut tail: Vec<u8> = self.tail.into();
        if self.omitted == 0 {
            head.extend_from_slice(&tail);
            return Kept {
                text: String::from_utf8_lossy(&head).into_owned(),
                omitted: 0,
            };
        }
        // The cut is made in bytes, and a character can be several: the
        // halves of one would each read as garbage.
        head.truncate(whole_chars_end(&head));
        tail.drain(..continuation_run(&tail));
        Kept {
            text: format!(
                "{}{GAP}{}",
                String::from_utf8_lossy(&head),
                String::from_utf8_lossy(&tail)
            ),
            omitted: self.omitted,
        }
    }
}

/// How many bytes a UTF-8 character starting with `lead` takes.
fn char_len(lead: u8) -> usize {
    match lead {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

/// Where `bytes` stops being whole characters: before a last character
/// that was cut short, or at the end.
fn whole_chars_end(bytes: &[u8]) -> usize {
    let from = bytes.len().saturating_sub(3);
    for at in (from..bytes.len()).rev() {
        if bytes[at] >= 0xC0 {
            return if at + char_len(bytes[at]) > bytes.len() { at } else { bytes.len() };
        }
        if bytes[at] < 0x80 {
            break;
        }
    }
    bytes.len()
}

/// How many bytes at the start of `bytes` belong to a character that
/// began before them.
fn continuation_run(bytes: &[u8]) -> usize {
    bytes.iter().take(3).take_while(|b| (0x80..0xC0).contains(*b)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(max: usize, chunks: &[&[u8]]) -> Kept {
        let mut tape = Tape::new(max);
        for chunk in chunks {
            tape.push(chunk);
        }
        tape.finish()
    }

    #[test]
    fn output_within_the_budget_is_kept_whole() {
        let out = kept(100, &[b"On branch main\n"]);
        assert_eq!(out.text, "On branch main\n");
        assert_eq!(out.omitted, 0);
    }

    #[test]
    fn output_arrives_in_pieces_and_reads_as_one() {
        let out = kept(100, &[b"one\n", b"two\n", b"three\n"]);
        assert_eq!(out.text, "one\ntwo\nthree\n");
        assert_eq!(out.omitted, 0);
    }

    #[test]
    fn output_exactly_at_the_budget_drops_nothing() {
        let out = kept(10, &[b"0123456789"]);
        assert_eq!(out.text, "0123456789");
        assert_eq!(out.omitted, 0);
    }

    #[test]
    fn past_the_budget_the_start_and_the_end_stay_and_the_middle_is_counted() {
        let out = kept(10, &[b"AAAAA", b"xxxxxxx", b"ZZZZZ"]);
        assert_eq!(out.text, format!("AAAAA{GAP}ZZZZZ"));
        assert_eq!(out.omitted, 7);
    }

    #[test]
    fn the_end_is_the_last_thing_printed_however_many_pieces_came() {
        let mut tape = Tape::new(8);
        tape.push(b"head");
        for n in 0..100 {
            tape.push(format!("{n:03}").as_bytes());
        }
        let out = tape.finish();
        assert_eq!(out.text, format!("head{GAP}8099"));
        assert_eq!(out.omitted, 300 - 4);
    }

    #[test]
    fn one_piece_larger_than_the_whole_budget_is_cut_the_same_way() {
        let out = kept(6, &[b"abcdefghijkl"]);
        assert_eq!(out.text, format!("abc{GAP}jkl"));
        assert_eq!(out.omitted, 6);
    }

    #[test]
    fn a_character_the_cut_went_through_is_dropped_not_garbled() {
        // "é" is two bytes: three bytes from the start end inside the
        // second one, three from the end begin inside the one before last.
        let out = kept(6, &["éé--------éé".as_bytes()]);
        assert_eq!(out.text, format!("é{GAP}é"));
        assert!(!out.text.contains('\u{fffd}'));
    }

    #[test]
    fn bytes_that_are_no_text_at_all_still_come_out_as_text() {
        let out = kept(100, &[&[b'o', b'k', 0xff, b'\n']]);
        assert_eq!(out.text, "ok\u{fffd}\n");
    }
}
