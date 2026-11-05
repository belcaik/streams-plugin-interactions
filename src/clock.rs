use std::time::Instant;

pub trait Clock {
    fn now_ms(&self) -> u64;
}

pub struct SystemClock {
    origin: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        self.origin
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
#[derive(Clone)]
pub struct ManualClock {
    now: std::cell::Cell<u64>,
}

#[cfg(test)]
impl ManualClock {
    pub fn new(now: u64) -> Self {
        Self {
            now: std::cell::Cell::new(now),
        }
    }

    pub fn set(&self, now: u64) {
        self.now.set(now);
    }

    pub fn advance(&self, milliseconds: u64) {
        self.now.set(self.now.get().saturating_add(milliseconds));
    }
}

#[cfg(test)]
impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.now.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_sets_and_advances_without_sleeping() {
        let clock = ManualClock::new(10);
        clock.advance(5);
        clock.set(30);
        assert_eq!(clock.now_ms(), 30);
    }
}
