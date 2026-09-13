//! What you typed while the agent was still working.
//!
//! The CLI has a queue of its own: write to its stdin mid-turn and it runs
//! that text when the turn ends. That queue is invisible and final — you
//! cannot see what is waiting in it, you cannot drop a message you regret,
//! and you cannot push one ahead of the turn that is running. Holding the
//! messages HERE is what buys those three.

/// A message waiting for its turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub id: String,
    pub text: String,
    /// `(media_type, base64)` — carried through untouched.
    pub images: Vec<(String, String)>,
}

impl Held {
    pub fn new(id: &str, text: &str) -> Self {
        Self { id: id.into(), text: text.into(), images: Vec::new() }
    }
}

/// What the shell should do with a message it was just handed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// Nothing is running: send it.
    SendNow(Held),
    /// A turn is running: it waits, visibly.
    Held,
}

#[derive(Debug, Default, Clone)]
pub struct Outbox {
    in_flight: bool,
    held: Vec<Held>,
}

impl Outbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// A turn started by something other than `offer` (the opening
    /// instruction, a restart).
    pub fn start(&mut self) {
        self.in_flight = true;
    }

    pub fn in_flight(&self) -> bool {
        self.in_flight
    }

    pub fn held(&self) -> &[Held] {
        &self.held
    }

    pub fn offer(&mut self, msg: Held) -> Offer {
        if self.in_flight {
            self.held.push(msg);
            return Offer::Held;
        }
        self.in_flight = true;
        Offer::SendNow(msg)
    }

    /// "Send this one now". It does NOT leave the queue: the caller cuts
    /// the running turn, and cutting it produces a result like any other
    /// ending, which drains the queue from the front. Handing the message
    /// straight back instead would race that drain — two turns, one
    /// worker. Jumping the line is the whole act.
    pub fn promote(&mut self, id: &str) -> bool {
        match self.held.iter().position(|h| h.id == id) {
            Some(at) => {
                let msg = self.held.remove(at);
                self.held.insert(0, msg);
                true
            }
            None => false,
        }
    }

    /// Drop one without sending it.
    pub fn discard(&mut self, id: &str) -> bool {
        match self.held.iter().position(|h| h.id == id) {
            Some(at) => {
                self.held.remove(at);
                true
            }
            None => false,
        }
    }

    /// The turn ended. Returns the next message to send, if any; the rest
    /// stay held, because only one turn runs at a time.
    pub fn finish(&mut self, join: bool) -> Option<Held> {
        if self.held.is_empty() {
            self.in_flight = false;
            return None;
        }
        if !join {
            return Some(self.held.remove(0));
        }
        let all = std::mem::take(&mut self.held);
        let mut merged = all[0].clone();
        merged.text = all.iter().map(|h| h.text.as_str()).collect::<Vec<_>>().join("\n\n");
        merged.images = all.into_iter().flat_map(|h| h.images).collect();
        Some(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_outbox_sends_straight_out_and_marks_the_turn() {
        let mut out = Outbox::new();
        let offer = out.offer(Held::new("a", "roda os testes"));
        assert_eq!(offer, Offer::SendNow(Held::new("a", "roda os testes")));
        assert!(out.in_flight(), "the message it just released IS a turn in flight");
        assert!(out.held().is_empty());
    }

    #[test]
    fn a_turn_in_flight_holds_what_you_type_in_order() {
        let mut out = Outbox::new();
        out.start();
        assert_eq!(out.offer(Held::new("a", "primeira")), Offer::Held);
        assert_eq!(out.offer(Held::new("b", "segunda")), Offer::Held);
        let ids: Vec<&str> = out.held().iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
    }

    #[test]
    fn promoting_jumps_the_line_and_goes_out_on_the_next_ending() {
        let mut out = Outbox::new();
        out.start();
        out.offer(Held::new("a", "primeira"));
        out.offer(Held::new("b", "segunda"));
        assert!(out.promote("b"));
        let ids: Vec<&str> = out.held().iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["b", "a"]);
        assert!(out.in_flight(), "the turn is being cut, not ended — that is the caller's job");
        assert_eq!(out.finish(false), Some(Held::new("b", "segunda")));
    }

    #[test]
    fn promoting_an_id_that_is_not_there_changes_nothing() {
        let mut out = Outbox::new();
        out.start();
        out.offer(Held::new("a", "primeira"));
        out.offer(Held::new("b", "segunda"));
        assert!(!out.promote("ghost"));
        let ids: Vec<&str> = out.held().iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
    }

    #[test]
    fn discarding_removes_only_that_message() {
        let mut out = Outbox::new();
        out.start();
        out.offer(Held::new("a", "primeira"));
        out.offer(Held::new("b", "segunda"));
        assert!(out.discard("a"));
        assert!(!out.discard("a"), "already gone");
        let ids: Vec<&str> = out.held().iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["b"]);
    }

    #[test]
    fn finishing_an_empty_queue_ends_the_flight() {
        let mut out = Outbox::new();
        out.start();
        assert!(out.in_flight(), "start() is what a spawn or a restart reports");
        assert_eq!(out.finish(false), None);
        assert!(!out.in_flight(), "nothing waiting: the worker is idle again");
    }

    #[test]
    fn finishing_releases_one_message_and_keeps_the_rest_waiting() {
        let mut out = Outbox::new();
        out.start();
        out.offer(Held::new("a", "primeira"));
        out.offer(Held::new("b", "segunda"));
        assert_eq!(out.finish(false), Some(Held::new("a", "primeira")));
        let ids: Vec<&str> = out.held().iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["b"], "only one turn runs at a time");
        assert!(out.in_flight(), "the message it just released IS the next turn");
    }

    #[test]
    fn joining_delivers_everything_as_one_turn() {
        let mut out = Outbox::new();
        out.start();
        out.offer(Held::new("a", "primeira"));
        out.offer(Held::new("b", "segunda"));
        let sent = out.finish(true).expect("something was waiting");
        assert_eq!(sent.text, "primeira\n\nsegunda");
        assert_eq!(sent.id, "a", "the merged turn answers to the first message's card");
        assert!(out.held().is_empty());
        assert!(out.in_flight());
    }

    #[test]
    fn images_ride_along_with_the_text_they_were_typed_with() {
        let mut out = Outbox::new();
        out.start();
        let mut msg = Held::new("a", "olha esse print");
        msg.images.push(("image/png".into(), "AAAA".into()));
        out.offer(msg.clone());
        assert_eq!(out.finish(false), Some(msg));
    }
}
