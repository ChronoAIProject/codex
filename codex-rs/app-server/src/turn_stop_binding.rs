use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

use codex_app_server_protocol::TurnStopStatus;
use codex_app_server_protocol::TurnStopStatusParams;

static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("CHRONO_CODEX_EXACT_TURN_STOP_V1").as_deref() == Ok("1"));
const RETENTION: Duration = Duration::from_secs(60 * 60);

pub(crate) fn enabled() -> bool {
    *ENABLED
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DispatchBinding {
    pub(crate) listener_incarnation: String,
    pub(crate) dispatch_sequence: u32,
    pub(crate) dispatch_key: String,
}

#[derive(Default)]
pub(crate) struct TurnStopBinding {
    start: Option<DispatchBinding>,
    accepted: Option<AcceptedTurn>,
}

struct AcceptedTurn {
    binding: DispatchBinding,
    turn_id: String,
    expires_at: Instant,
    canonical: bool,
    stop: StopAttempt,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StopAttempt {
    Unrequested,
    Requested,
    Unconfirmed,
}

impl TurnStopBinding {
    pub(crate) fn reserve_start(&mut self, binding: DispatchBinding) -> bool {
        if binding.dispatch_sequence == 0
            || !(1..=128).contains(&binding.dispatch_key.len())
            || !binding
                .dispatch_key
                .bytes()
                .all(|byte| byte.is_ascii_graphic())
            || self.start.as_ref().is_some_and(|previous| {
                previous.listener_incarnation == binding.listener_incarnation
                    && (previous.dispatch_sequence >= binding.dispatch_sequence
                        || previous.dispatch_key == binding.dispatch_key)
            })
        {
            return false;
        }
        self.start = Some(binding);
        true
    }

    pub(crate) fn complete_start(
        &mut self,
        listener_incarnation: &str,
        dispatch_sequence: u32,
        turn_id: String,
        canonical: bool,
    ) {
        if let Some(start) = self.start.as_ref().filter(|start| {
            start.listener_incarnation == listener_incarnation
                && start.dispatch_sequence == dispatch_sequence
        }) {
            self.accepted = Some(AcceptedTurn {
                binding: start.clone(),
                turn_id,
                expires_at: Instant::now() + RETENTION,
                canonical,
                stop: StopAttempt::Unrequested,
            });
        }
    }

    pub(crate) fn note_canonical(&mut self, turn_id: &str) {
        if let Some(accepted) = &mut self.accepted
            && accepted.turn_id == turn_id
        {
            accepted.canonical = true;
        }
    }

    pub(crate) fn observe(
        &self,
        listener_incarnation: &str,
        dispatch_sequence: u32,
        dispatch_key: &str,
        turn_id: &str,
    ) -> TurnStopStatus {
        let Some(accepted) = &self.accepted else {
            return TurnStopStatus::Hold;
        };
        if accepted.binding.listener_incarnation != listener_incarnation
            || accepted.binding.dispatch_sequence != dispatch_sequence
            || accepted.binding.dispatch_key != dispatch_key
            || accepted.turn_id != turn_id
            || Instant::now() >= accepted.expires_at
        {
            return TurnStopStatus::Hold;
        }
        if accepted.canonical {
            TurnStopStatus::Canonical
        } else if accepted.stop == StopAttempt::Unconfirmed {
            TurnStopStatus::Hold
        } else {
            TurnStopStatus::Pending
        }
    }

    pub(crate) fn status(&self, target: &TurnStopStatusParams) -> TurnStopStatus {
        self.observe(
            &target.listener_incarnation,
            target.dispatch_sequence,
            &target.dispatch_key,
            &target.turn_id,
        )
    }

    pub(crate) fn claim_stop(&mut self, target: &TurnStopStatusParams) -> bool {
        if self.status(target) != TurnStopStatus::Pending {
            return false;
        }
        let Some(accepted) = &mut self.accepted else {
            return false;
        };
        if accepted.stop != StopAttempt::Unrequested {
            return false;
        }
        accepted.stop = StopAttempt::Requested;
        true
    }

    pub(crate) fn note_unconfirmed_stop(&mut self, target: &TurnStopStatusParams) {
        if self.status(target) == TurnStopStatus::Pending
            && let Some(accepted) = &mut self.accepted
            && accepted.stop == StopAttempt::Requested
        {
            accepted.stop = StopAttempt::Unconfirmed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(sequence: u32, listener: &str) -> DispatchBinding {
        DispatchBinding {
            listener_incarnation: listener.to_string(),
            dispatch_sequence: sequence,
            dispatch_key: format!("key-{sequence}"),
        }
    }

    #[test]
    fn binds_at_dispatch_and_preserves_canonical_result() {
        let mut state = TurnStopBinding::default();
        assert_eq!(
            state.observe("listener", 1, "key-1", "turn"),
            TurnStopStatus::Hold
        );
        assert!(state.reserve_start(binding(1, "listener")));
        assert!(!state.reserve_start(binding(1, "listener")));
        state.complete_start("listener", 1, "turn".into(), false);
        assert_eq!(
            state.observe("listener", 1, "key-1", "turn"),
            TurnStopStatus::Pending
        );
        assert_eq!(
            state.observe("listener", 1, "wrong", "turn"),
            TurnStopStatus::Hold
        );
        assert_eq!(
            state.observe("listener", 1, "key-1", "successor"),
            TurnStopStatus::Hold
        );
        assert_eq!(
            state.observe("replacement", 1, "key-1", "turn"),
            TurnStopStatus::Hold
        );
        state.note_canonical("successor");
        assert_eq!(
            state.observe("listener", 1, "key-1", "turn"),
            TurnStopStatus::Pending
        );
        state.note_canonical("turn");
        for _ in 0..2 {
            assert_eq!(
                state.observe("listener", 1, "key-1", "turn"),
                TurnStopStatus::Canonical
            );
        }
        assert!(state.reserve_start(binding(2, "listener")));
        assert!(!state.reserve_start(binding(1, "listener")));
        assert_eq!(
            state.observe("listener", 1, "key-1", "turn"),
            TurnStopStatus::Canonical
        );
        state.complete_start("listener", 2, "successor".into(), false);
        assert_eq!(
            state.observe("listener", 1, "key-1", "turn"),
            TurnStopStatus::Hold
        );
    }

    #[test]
    fn late_start_response_and_expired_result_fail_closed() {
        let mut state = TurnStopBinding::default();
        assert!(state.reserve_start(binding(1, "listener")));
        assert!(state.reserve_start(binding(2, "replacement")));
        state.complete_start("listener", 1, "old".into(), true);
        assert_eq!(
            state.observe("listener", 1, "key-1", "old"),
            TurnStopStatus::Hold
        );
        state.complete_start("replacement", 2, "turn".into(), true);
        assert_eq!(
            state.observe("replacement", 2, "key-2", "turn"),
            TurnStopStatus::Canonical
        );
        state.accepted.as_mut().expect("accepted").expires_at = Instant::now();
        assert_eq!(
            state.observe("replacement", 2, "key-2", "turn"),
            TurnStopStatus::Hold
        );
    }

    #[test]
    fn claims_exact_stop_once_and_keeps_competing_canonical_result() {
        let mut state = TurnStopBinding::default();
        assert!(state.reserve_start(binding(1, "listener")));
        state.complete_start("listener", 1, "turn".into(), false);
        let mut target = TurnStopStatusParams {
            thread_id: "thread".into(),
            turn_id: "turn".into(),
            listener_incarnation: "listener".into(),
            dispatch_sequence: 1,
            dispatch_key: "wrong".into(),
        };
        assert!(!state.claim_stop(&target));
        target.dispatch_key = "key-1".into();
        assert!(state.claim_stop(&target));
        assert!(!state.claim_stop(&target));
        state.note_unconfirmed_stop(&target);
        assert_eq!(state.status(&target), TurnStopStatus::Hold);
        state.note_canonical("turn");
        assert_eq!(state.status(&target), TurnStopStatus::Canonical);
        assert!(!state.claim_stop(&target));
    }
}
