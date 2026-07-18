mod coordinator;
mod reconcile;

pub use coordinator::{CoordinatorRecord, PreflightCoordinator, PreflightDirective};
pub use reconcile::{
    project_monitor, reconcile_attempt, EligibilityKey, NativeObservation, PreflightAttempt,
    PreflightInput, PreflightOutcome, PreflightPhase, PreflightReason, PreflightSignal,
};
