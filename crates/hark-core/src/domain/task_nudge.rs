//! The soft reminder that a chat has no task. A task is a deliberate
//! record (05/11): the window never creates one, it may suggest one —
//! once per chat, after the conversation has some body, never as a
//! modal. Pure: the shell counts, this decides.

/// A chat with fewer turns of yours than this is small talk still.
pub const MIN_USER_TURNS: usize = 3;

/// Whether the line "este chat não tem task · registrar?" shows.
pub fn offer(user_turns: usize, has_task: bool, dismissed: bool) -> bool {
    !has_task && !dismissed && user_turns >= MIN_USER_TURNS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_with_some_body_and_no_task_gets_the_offer_once() {
        assert!(offer(3, false, false));
        assert!(!offer(2, false, false), "two turns is still small talk");
        assert!(!offer(5, true, false), "it has a task already");
        assert!(!offer(5, false, true), "the person said not now");
    }
}
