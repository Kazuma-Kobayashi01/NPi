//! The user's go-ahead for a review: Rust shows a card (`review://confirm`) and waits for the answer
//! from the page (`expert_review_answer`), not from the model, so a review never starts on the
//! model's word alone. No answer in time, a closed tab or a page that cannot show it is a no.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

/// How long a card waits for the user.
pub const WAIT: Duration = Duration::from_mins(10);

static PENDING: Mutex<Option<HashMap<String, Sender<bool>>>> = Mutex::new(None);

fn pending<R>(f: impl FnOnce(&mut HashMap<String, Sender<bool>>) -> R) -> R {
    let mut guard = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
    f(guard.get_or_insert_with(HashMap::new))
}

/// Shows the card through `show` (false when it could not be shown) and waits up to `wait` for the
/// answer to `id`. True only for an explicit yes.
pub fn ask(id: &str, show: impl FnOnce() -> bool, wait: Duration) -> bool {
    let (tx, rx) = channel();
    pending(|p| p.insert(id.to_string(), tx));
    let yes = show() && rx.recv_timeout(wait).unwrap_or(false);
    pending(|p| p.remove(id));
    yes
}

/// The page's answer for card `id`; false when no card of that id is waiting.
pub fn answer(id: &str, accept: bool) -> bool {
    pending(|p| p.remove(id)).is_some_and(|tx| tx.send(accept).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_explicit_yes_in_time_goes_ahead() {
        let t = std::thread::spawn(|| ask("r1", || true, Duration::from_secs(5)));
        while !pending(|p| p.contains_key("r1")) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(answer("r1", true));
        assert!(t.join().unwrap());

        assert!(!ask("r2", || true, Duration::from_millis(20)), "no answer is a no");
        assert!(!ask("r3", || false, Duration::from_secs(5)), "a card that could not be shown is a no");
        assert!(!answer("r3", true), "nothing is waiting any more");
        assert!(!answer("nobody", true));
    }
}
