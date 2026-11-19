//! Startup hook: the session recovery that must finish before Play is accepted.
use crate::{AppState, CommandError};

/// Cleans a session left deployed by a crash and records the amount. A
/// failure is already reported on the error channel and keeps new deploys
/// blocked; it is returned so the caller can log it.
pub fn run_recovery(state: &AppState) -> Result<usize, CommandError> {
    state.play.recover_on_startup().map_err(CommandError::from)
}
