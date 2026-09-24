//! Serializes calls to an outbound API at a minimum interval.

use simple_server::rate_limit::{Budget, Quota};
use std::num::NonZeroU32;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(super) struct RequestPacer {
    origin: Instant,
    budget: Mutex<Budget>,
}

impl RequestPacer {
    pub(super) fn new(interval: Duration) -> Self {
        let quota = Quota::replenishing(interval, NonZeroU32::new(1).unwrap())
            .expect("outbound request interval is positive");
        Self {
            origin: Instant::now(),
            budget: Mutex::new(Budget::new(quota)),
        }
    }

    pub(super) fn wait(&self) -> Duration {
        self.wait_with(|| self.origin.elapsed(), std::thread::sleep)
    }

    fn wait_with<Clock, Sleeper>(&self, mut now: Clock, mut sleep: Sleeper) -> Duration
    where
        Clock: FnMut() -> Duration,
        Sleeper: FnMut(Duration),
    {
        // Hold the lock across the wait. The next caller must measure its own
        // delay after this call has charged the budget at the actual wake time.
        let mut budget = self.budget.lock().unwrap();
        loop {
            let observed = now();
            match budget.check_at(observed, NonZeroU32::new(1).unwrap()) {
                Ok(()) => return observed,
                Err(rejection) => sleep(
                    rejection
                        .retry_after
                        .expect("a positive one-request interval has a retry time"),
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::sync::Arc;

    #[test]
    fn first_call_is_immediate_and_next_call_waits_exact_interval() {
        let interval = Duration::from_millis(100);
        let pacer = RequestPacer::new(interval);
        let clock = Cell::new(Duration::ZERO);
        let sleeps = RefCell::new(Vec::new());
        let admit = || {
            pacer.wait_with(
                || clock.get(),
                |delay| {
                    sleeps.borrow_mut().push(delay);
                    clock.set(clock.get() + delay);
                },
            )
        };
        assert_eq!(admit(), Duration::ZERO);
        assert!(sleeps.borrow().is_empty());
        assert_eq!(admit(), interval);
        assert_eq!(*sleeps.borrow(), vec![interval]);
    }

    #[test]
    fn concurrent_calls_are_charged_at_spaced_instants() {
        let interval = Duration::from_millis(20);
        let pacer = Arc::new(RequestPacer::new(interval));
        let first = pacer.wait();

        let admitted = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..3)
                .map(|_| {
                    let pacer = pacer.clone();
                    scope.spawn(move || pacer.wait())
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .collect::<Vec<_>>()
        });
        let mut admitted = admitted;
        admitted.sort_unstable();
        assert!(admitted[0] - first >= interval);
        assert!(admitted
            .windows(2)
            .all(|pair| pair[1] - pair[0] >= interval));
    }
}
