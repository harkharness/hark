//! Something loaded once and kept for the life of the process, that can
//! still be let go before the process ends.
//!
//! The speech model lived in a `static`, which nothing ever drops. On
//! quitting, the Metal backend's own teardown found the model's buffers
//! still registered and aborted (`GGML_ASSERT([rsets->data count] == 0)`):
//! every quit of an app that had loaded the model ended as a crash, exit
//! status 134. Held here, the model is released on the way out, and the
//! teardown finds nothing left.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub struct Held<T> {
    slot: RwLock<Option<Arc<T>>>,
}

impl<T> Held<T> {
    pub const fn new() -> Self {
        Self { slot: RwLock::new(None) }
    }

    /// Keep `value`. The first one kept stays: false when there was one.
    pub fn set(&self, value: T) -> bool {
        let mut slot = self.slot.write().unwrap_or_else(|e| e.into_inner());
        if slot.is_some() {
            return false;
        }
        *slot = Some(Arc::new(value));
        true
    }

    pub fn is_set(&self) -> bool {
        self.slot.read().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    /// What is kept, for as long as the caller holds on to it.
    pub fn get(&self) -> Option<Arc<T>> {
        self.slot.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Let it go. Whoever is in the middle of using it gets `patience` to
    /// finish; true when it was dropped (or there was nothing kept).
    ///
    /// It leaves the slot at once, so nobody new picks it up while the
    /// wait lasts. When the patience runs out it is not pulled from under
    /// its user: it goes when that use ends, and the answer is false.
    pub fn release(&self, patience: Duration) -> bool {
        let Some(mut mine) = self.slot.write().unwrap_or_else(|e| e.into_inner()).take() else {
            return true;
        };
        let until = Instant::now() + patience;
        loop {
            match Arc::try_unwrap(mine) {
                Ok(value) => {
                    drop(value);
                    return true;
                }
                Err(_) if Instant::now() >= until => return false,
                Err(shared) => {
                    mine = shared;
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Says when it was dropped.
    struct Probe(Arc<AtomicBool>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn probe() -> (Probe, Arc<AtomicBool>) {
        let dropped = Arc::new(AtomicBool::new(false));
        (Probe(dropped.clone()), dropped)
    }

    const LONG: Duration = Duration::from_secs(5);

    #[test]
    fn nothing_is_kept_until_something_is() {
        let held: Held<u32> = Held::new();
        assert!(!held.is_set());
        assert!(held.get().is_none());
    }

    #[test]
    fn what_was_kept_is_handed_out() {
        let held = Held::new();
        assert!(held.set(7u32));
        assert!(held.is_set());
        assert_eq!(held.get().as_deref(), Some(&7));
    }

    #[test]
    fn the_first_one_kept_stays() {
        // Two loads racing: the second is dropped, not swapped in under
        // someone who is using the first.
        let held = Held::new();
        assert!(held.set(1u32));
        assert!(!held.set(2u32));
        assert_eq!(held.get().as_deref(), Some(&1));
    }

    #[test]
    fn releasing_drops_what_was_kept() {
        let held = Held::new();
        let (value, dropped) = probe();
        held.set(value);
        assert!(!dropped.load(Ordering::SeqCst));
        assert!(held.release(LONG));
        assert!(dropped.load(Ordering::SeqCst), "it is gone before release returns");
        assert!(!held.is_set());
        assert!(held.get().is_none());
    }

    #[test]
    fn releasing_nothing_is_done_at_once() {
        let held: Held<u32> = Held::new();
        let began = Instant::now();
        assert!(held.release(LONG));
        assert!(began.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn releasing_waits_for_whoever_is_still_using_it() {
        let held = Held::new();
        let (value, dropped) = probe();
        held.set(value);
        let in_use = held.get().unwrap();
        let user = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            drop(in_use);
        });
        let began = Instant::now();
        assert!(held.release(LONG));
        assert!(began.elapsed() >= Duration::from_millis(150), "it waited for the use to end");
        assert!(dropped.load(Ordering::SeqCst));
        user.join().unwrap();
    }

    #[test]
    fn releasing_gives_up_on_a_use_that_never_ends_and_says_so() {
        let held = Held::new();
        let (value, dropped) = probe();
        held.set(value);
        let in_use = held.get().unwrap();
        let began = Instant::now();
        assert!(!held.release(Duration::from_millis(150)));
        assert!(began.elapsed() < Duration::from_secs(2), "the way out is never held up for long");
        assert!(!dropped.load(Ordering::SeqCst), "never pulled from under its user");
        // Nobody new gets it meanwhile, and it goes when its user is done.
        assert!(held.get().is_none());
        drop(in_use);
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn something_can_be_kept_again_after_a_release() {
        let held = Held::new();
        held.set(1u32);
        held.release(LONG);
        assert!(held.set(2u32));
        assert_eq!(held.get().as_deref(), Some(&2));
    }
}
