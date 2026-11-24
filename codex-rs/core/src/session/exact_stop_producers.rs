use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;

const OPEN: u8 = 0;
const UNTRACKED: u8 = 1;
const CLOSED: u8 = 2;

/// A turn cannot prove a stop after it admits work whose lifetime is outside its task.
#[derive(Debug, Default)]
pub(crate) struct ExactStopProducers(AtomicU8);

impl ExactStopProducers {
    pub(crate) fn admit_untracked(&self) -> bool {
        self.0
            .try_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                (state != CLOSED).then_some(UNTRACKED)
            })
            .is_ok()
    }

    pub(crate) fn close(&self) -> bool {
        self.0
            .compare_exchange(OPEN, CLOSED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untracked_work_disqualifies_the_turn_after_its_queue_drains() {
        let producers = ExactStopProducers::default();
        assert!(producers.admit_untracked());
        assert!(!producers.close());
        assert!(producers.admit_untracked());
    }

    #[test]
    fn a_closed_turn_refuses_new_untracked_work() {
        let producers = ExactStopProducers::default();
        assert!(producers.close());
        assert!(!producers.admit_untracked());
        assert!(!producers.close());
    }
}
