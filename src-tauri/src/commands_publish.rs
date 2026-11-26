//! Thin Tauri commands of publishing mods to the community server (#130). The
//! upload runs on the blocking pool WITHOUT `write_lock` (it can take minutes
//! and touches no launcher store). The session token stays in the service: it
//! never crosses IPC.
use crate::commands::read_blocking;
use crate::dto_publish::{ModVersionDto, MyModDto, PublishProgressDto, PublishRequestDto};
use crate::{AppState, CommandError};
use motogp_application::{CancelFlag, PublishError, PublishPhase, PublishProgress};
use tauri::{AppHandle, Emitter, Runtime, State};

/// Event name of publish progress.
pub const COMMUNITY_PUBLISH_PROGRESS_EVENT: &str = "community-publish-progress";

/// Removes the cancel flag of the publish when it ends, however it ends.
struct CancelRegistration<'a> {
    state: &'a AppState,
}

impl<'a> CancelRegistration<'a> {
    /// `None` when a publish is already running.
    fn register(state: &'a AppState, flag: &CancelFlag) -> Option<Self> {
        let mut slot = state
            .publish_cancel
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if slot.is_some() {
            return None;
        }
        *slot = Some(flag.clone());
        Some(Self { state })
    }
}

impl Drop for CancelRegistration<'_> {
    fn drop(&mut self) {
        *self
            .state
            .publish_cancel
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Lets about 100 updates per phase through (hashing reports every 256 KiB).
/// A phase change and the end of a phase always pass.
#[derive(Debug, Default)]
struct ProgressThrottle {
    last: Option<(PublishPhase, u64)>,
}

impl ProgressThrottle {
    fn allow(&mut self, progress: &PublishProgress) -> bool {
        let step = (progress.total / 100).max(1);
        let pass = match self.last {
            Some((phase, sent)) if phase == progress.phase => {
                progress.sent >= progress.total || progress.sent.abs_diff(sent) >= step
            }
            _ => true,
        };
        if pass {
            self.last = Some((progress.phase, progress.sent));
        }
        pass
    }
}

fn emit<R: Runtime>(app: &AppHandle<R>, progress: PublishProgress) {
    if let Err(error) = app.emit(
        COMMUNITY_PUBLISH_PROGRESS_EVENT,
        PublishProgressDto::from(progress),
    ) {
        eprintln!("could not emit the publish progress: {error}");
    }
}

/// Uploads a mod and leaves it waiting for review. Emits
/// `community-publish-progress` `{sent, total, phase}`. Only one publish runs at
/// a time (`invalid_request` otherwise).
#[tauri::command]
pub async fn community_publish_mod<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    request: PublishRequestDto,
) -> Result<ModVersionDto, CommandError> {
    let cancel = CancelFlag::new();
    let _registration = CancelRegistration::register(state.inner(), &cancel).ok_or_else(|| {
        PublishError::InvalidRequest("another upload is already running".to_string())
    })?;
    read_blocking(state.clone(), move |state| {
        let mut throttle = ProgressThrottle::default();
        let mut report = |progress: PublishProgress| {
            if throttle.allow(&progress) {
                emit(&app, progress);
            }
        };
        let version = state
            .publish
            .publish(&request.into(), &mut report, &cancel)?;
        Ok(version.into())
    })
    .await
}

/// Asks the running publish to stop and abort its upload. A no-op when none runs.
#[tauri::command]
pub fn community_cancel_publish(state: State<'_, AppState>) {
    let slot = state
        .publish_cancel
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(flag) = slot.as_ref() {
        flag.cancel();
    }
}

/// The signed-in creator's mods with every version and its review status.
#[tauri::command]
pub async fn community_my_mods(state: State<'_, AppState>) -> Result<Vec<MyModDto>, CommandError> {
    read_blocking(state, |state| {
        Ok(state
            .publish
            .my_mods()?
            .into_iter()
            .map(MyModDto::from)
            .collect())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(phase: PublishPhase, sent: u64, total: u64) -> PublishProgress {
        PublishProgress { phase, sent, total }
    }

    #[test]
    fn the_throttle_lets_about_one_percent_steps_through() {
        let mut throttle = ProgressThrottle::default();
        let total = 1000;
        let passed = (0..=total)
            .filter(|sent| throttle.allow(&progress(PublishPhase::Hashing, *sent, total)))
            .count();
        // The first report, then one every 10 bytes: about 100, never 1000.
        assert!((100..=102).contains(&passed), "{passed}");
    }

    #[test]
    fn a_phase_change_and_the_end_of_a_phase_always_pass() {
        let mut throttle = ProgressThrottle::default();
        assert!(throttle.allow(&progress(PublishPhase::Hashing, 0, 1000)));
        assert!(!throttle.allow(&progress(PublishPhase::Hashing, 1, 1000)));
        assert!(throttle.allow(&progress(PublishPhase::Hashing, 1000, 1000)));
        assert!(throttle.allow(&progress(PublishPhase::Uploading, 1, 1000)));
        assert!(throttle.allow(&progress(PublishPhase::Done, 1000, 1000)));
    }

    #[test]
    fn a_tiny_total_never_divides_by_zero() {
        let mut throttle = ProgressThrottle::default();
        assert!(throttle.allow(&progress(PublishPhase::Uploading, 0, 3)));
        assert!(throttle.allow(&progress(PublishPhase::Uploading, 1, 3)));
    }
}
