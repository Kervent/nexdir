//! Bounded execution of blocking network calls.
//!
//! Windows network functions (`WNet*`, opening a path on an unreachable server) can block for
//! a long time, and there is no way to cancel such a call from outside. Nexdir therefore runs
//! each call on its own thread and only *waits* with a timeout: when the timeout expires the
//! caller gets `TimedOut` immediately, while the thread keeps running until Windows returns.
//!
//! To keep hung calls from piling up, a `Gate` admits only a fixed number of calls at once.
//! A slot is released when the call really returns, not when the caller stops waiting. If every
//! slot is taken by a hung call, new calls are refused with `Busy` instead of starting more
//! threads. So at most `slots` threads can ever be blocked in network calls.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateError {
    /// All slots are occupied by calls that have not returned yet.
    Busy,
    /// The call did not return in time; it continues in the background.
    TimedOut,
}

pub struct Gate {
    slots: usize,
    in_flight: AtomicUsize,
}

/// Releases the slot when the call returns (also on panic).
struct Slot<'a>(&'a AtomicUsize);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Gate {
    pub const fn new(slots: usize) -> Self {
        Self {
            slots,
            in_flight: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    fn acquire(&'static self) -> Result<Slot<'static>, GateError> {
        let mut current = self.in_flight.load(Ordering::SeqCst);
        loop {
            if current >= self.slots {
                return Err(GateError::Busy);
            }
            match self
                .in_flight
                .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return Ok(Slot(&self.in_flight)),
                Err(now) => current = now,
            }
        }
    }

    /// Runs `f` on a dedicated thread and waits at most `timeout` for its result.
    pub fn run<T: Send + 'static>(&'static self, timeout: Duration, f: impl FnOnce() -> T + Send + 'static) -> Result<T, GateError> {
        let slot = self.acquire()?;
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("nexdir-network".into()).spawn(move || {
            let _slot = slot;
            let _ = tx.send(f());
        });
        if spawned.is_err() {
            // The closure (and with it the slot) was dropped, so the slot is free again.
            return Err(GateError::Busy);
        }
        rx.recv_timeout(timeout).map_err(|_| GateError::TimedOut)
    }
}

/// Shared gate for all network calls (discovery, connect, status checks, listings of network
/// folders). Six slots: enough for a few slow servers at once, small enough to stay harmless.
pub static NETWORK: Gate = Gate::new(6);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn returns_results_in_time() {
        static G: Gate = Gate::new(2);
        assert_eq!(G.run(Duration::from_secs(5), || 42), Ok(42));
        // The slot is released once the call has returned.
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(G.in_flight(), 0);
    }

    #[test]
    fn times_out_without_waiting_for_a_hung_call() {
        static G: Gate = Gate::new(2);
        let release = Arc::new(Barrier::new(2));
        let r = release.clone();
        let t0 = std::time::Instant::now();
        assert_eq!(
            G.run(Duration::from_millis(100), move || r.wait()).map(|_| ()),
            Err(GateError::TimedOut)
        );
        assert!(t0.elapsed() < Duration::from_secs(2));
        // The hung call still holds its slot until it really returns.
        assert_eq!(G.in_flight(), 1);
        release.wait();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(G.in_flight(), 0);
    }

    #[test]
    fn refuses_new_calls_when_all_slots_hang() {
        static G: Gate = Gate::new(2);
        let release = Arc::new(Barrier::new(3));
        for _ in 0..2 {
            let r = release.clone();
            assert_eq!(
                G.run(Duration::from_millis(20), move || r.wait()).map(|_| ()),
                Err(GateError::TimedOut)
            );
        }
        // No third thread is started: the number of blocked threads stays bounded.
        assert_eq!(G.run(Duration::from_secs(1), || 1), Err(GateError::Busy));
        assert_eq!(G.in_flight(), 2);
        release.wait();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(G.run(Duration::from_secs(1), || 1), Ok(1));
    }

    #[test]
    fn a_panicking_call_releases_its_slot() {
        static G: Gate = Gate::new(1);
        assert_eq!(
            G.run(Duration::from_secs(1), || -> i32 { panic!("boom") }),
            Err(GateError::TimedOut)
        );
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(G.in_flight(), 0);
        assert_eq!(G.run(Duration::from_secs(1), || 7), Ok(7));
    }
}
