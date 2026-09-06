use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Draft,
    PendingTrigger,
    PendingRisk,
    Sending,
    Working,
    PartiallyFilled,
    Filled,
    Cancelling,
    Cancelled,
    Rejected,
    Expired,
    Error,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("illegal transition {from:?} -> {to:?}")]
pub struct IllegalTransition {
    pub from: Status,
    pub to: Status,
}

impl Status {
    pub fn can_transition(self, to: Status) -> bool {
        use Status::*;
        matches!(
            (self, to),
            (Draft, PendingTrigger | PendingRisk | Cancelled)
                | (PendingTrigger, PendingRisk | Cancelled | Expired)
                | (PendingRisk, Sending | Rejected | PendingTrigger)
                | (Sending, Working | Rejected | Error)
                | (Working, PartiallyFilled | Filled | Cancelling | Expired | Error)
                | (PartiallyFilled, PartiallyFilled | Filled | Cancelling | Expired)
                | (Cancelling, Cancelled | Filled | PartiallyFilled | Error)
        )
    }

    pub fn transition(self, to: Status) -> Result<Status, IllegalTransition> {
        if self.can_transition(to) {
            Ok(to)
        } else {
            Err(IllegalTransition { from: self, to })
        }
    }

    pub fn is_terminal(self) -> bool {
        use Status::*;
        matches!(self, Filled | Cancelled | Rejected | Expired | Error)
    }

    /// Borsada açık bir emir var mı (cancel gerektirir)
    pub fn is_live_at_broker(self) -> bool {
        use Status::*;
        matches!(self, Sending | Working | PartiallyFilled | Cancelling)
    }
}

#[cfg(test)]
mod tests {
    use super::Status::*;

    #[test]
    fn legal_paths() {
        assert!(Draft.can_transition(PendingRisk));
        assert!(PendingRisk.can_transition(Sending));
        assert!(Sending.can_transition(Working));
        assert!(Working.can_transition(PartiallyFilled));
        assert!(PartiallyFilled.can_transition(Filled));
    }

    #[test]
    fn illegal_paths() {
        assert!(!Filled.can_transition(Working));
        assert!(!Cancelled.can_transition(Sending));
        assert!(!Draft.can_transition(Working));
        assert_eq!(
            Filled.transition(Working).unwrap_err().to_string(),
            "illegal transition Filled -> Working"
        );
    }

    #[test]
    fn terminal() {
        for s in [Filled, Cancelled, Rejected, Expired, Error] {
            assert!(s.is_terminal());
        }
        assert!(!Working.is_terminal());
    }
}
