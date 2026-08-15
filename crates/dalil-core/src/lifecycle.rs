//! Host-controlled lifecycle dispatch for native Dalil integrations.
//!
//! Hosts call this module at their own lifecycle boundaries. It starts no
//! background work and only dispatches the matching ordinary Dalil operation.

use serde::{Deserialize, Serialize};

use crate::{AnalysisRequest, CommandName, CoreError, ExecutionControl, Report, analyze, analyze_with_control};

/// A host lifecycle boundary that can request a small advisory Dalil report.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleEvent {
    RepositoryOpen,
    SessionStart,
    TaskChange,
    BeforeEdit,
    AfterEdit,
    BeforeReview,
}

impl LifecycleEvent {
    /// The Dalil operation appropriate for this lifecycle boundary.
    pub const fn command(self) -> CommandName {
        match self {
            Self::RepositoryOpen | Self::SessionStart => CommandName::Orient,
            Self::TaskChange | Self::BeforeEdit | Self::AfterEdit => CommandName::Context,
            Self::BeforeReview => CommandName::Impact,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::RepositoryOpen => "repository_open",
            Self::SessionStart => "session_start",
            Self::TaskChange => "task_change",
            Self::BeforeEdit => "before_edit",
            Self::AfterEdit => "after_edit",
            Self::BeforeReview => "before_review",
        }
    }
}

/// One host-owned request at a lifecycle boundary.
///
/// The host supplies task, focus, changed-path, budget, and cache settings in
/// the embedded request. Dalil does not inspect editor buffers or session state.
#[derive(Debug)]
pub struct LifecycleRequest {
    event: LifecycleEvent,
    request: AnalysisRequest,
}

impl LifecycleRequest {
    /// Build a request after confirming that the selected operation fits the event.
    pub fn new(event: LifecycleEvent, request: AnalysisRequest) -> Result<Self, LifecycleError> {
        let expected = event.command();
        let actual = request.command.name;
        if actual != expected {
            return Err(LifecycleError::WrongOperation { event, expected: expected.label(), actual: actual.label() });
        }
        Ok(Self { event, request })
    }

    pub const fn event(&self) -> LifecycleEvent {
        self.event
    }

    pub const fn request(&self) -> &AnalysisRequest {
        &self.request
    }
}

/// The advisory report returned to a native host.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LifecycleNotice {
    pub event: LifecycleEvent,
    pub report: Report,
}

/// Errors at the lifecycle adapter boundary.
#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    #[error("lifecycle event `{event}` requires `{expected}`, received `{actual}`")]
    WrongOperation {
        event: LifecycleEvent,
        expected: &'static str,
        actual: &'static str,
    },
    #[error(transparent)]
    Core(#[from] CoreError),
}

impl std::fmt::Display for LifecycleEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Dispatch one lifecycle request synchronously.
///
/// This is equivalent to calling [`analyze`] with the embedded request. It does
/// not retain state or schedule follow-up work; the normal persistent index is
/// reused through the request's cache settings.
pub fn dispatch(request: LifecycleRequest) -> Result<LifecycleNotice, LifecycleError> {
    let event = request.event;
    let report = analyze(request.request)?;
    Ok(LifecycleNotice { event, report })
}

/// Dispatch one lifecycle request with host-provided cancellation and progress.
///
/// Cancellation is cooperative at the core operation boundaries. Hosts own the
/// token and decide when to discard a returned advisory notice.
pub fn dispatch_with_control(
    request: LifecycleRequest, control: &ExecutionControl,
) -> Result<LifecycleNotice, LifecycleError> {
    let event = request.event;
    let report = analyze_with_control(request.request, control)?;
    Ok(LifecycleNotice { event, report })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{AnalysisRequest, CommandDescriptor, ContextRequest, MapSettings, TaskChangeSeed};

    use super::*;

    #[test]
    fn events_select_task_shaped_operations() {
        assert_eq!(LifecycleEvent::RepositoryOpen.command(), CommandName::Orient);
        assert_eq!(LifecycleEvent::SessionStart.command(), CommandName::Orient);
        assert_eq!(LifecycleEvent::TaskChange.command(), CommandName::Context);
        assert_eq!(LifecycleEvent::BeforeEdit.command(), CommandName::Context);
        assert_eq!(LifecycleEvent::AfterEdit.command(), CommandName::Context);
        assert_eq!(LifecycleEvent::BeforeReview.command(), CommandName::Impact);
    }

    #[test]
    fn rejects_an_operation_that_cannot_produce_the_event_notice() {
        let request = AnalysisRequest::new(CommandDescriptor::orient(PathBuf::from(".")));

        assert!(matches!(
            LifecycleRequest::new(LifecycleEvent::BeforeReview, request),
            Err(LifecycleError::WrongOperation { expected: "impact", actual: "orient", .. })
        ));
    }

    #[test]
    fn dispatch_matches_direct_core_analysis() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("core crate is nested in the workspace")
            .to_path_buf();
        let direct = analyze(orientation_request(root.clone())).expect("direct orientation succeeds");
        let lifecycle = dispatch(
            LifecycleRequest::new(LifecycleEvent::RepositoryOpen, orientation_request(root))
                .expect("repository open accepts orientation"),
        )
        .expect("lifecycle orientation succeeds");

        assert_eq!(lifecycle.report, direct);
    }

    fn orientation_request(path: PathBuf) -> AnalysisRequest {
        let mut request = AnalysisRequest::new(CommandDescriptor::orient(path));
        request.map.cache_mode = crate::CacheMode::Disabled;
        request
    }

    #[test]
    fn cancelled_lifecycle_request_does_not_open_a_repository() {
        let request = LifecycleRequest::new(
            LifecycleEvent::RepositoryOpen,
            AnalysisRequest::new(CommandDescriptor::orient(PathBuf::from("not-a-repository"))),
        )
        .expect("orientation is valid at repository open");
        let cancellation = crate::CancellationToken::default();
        cancellation.cancel();

        assert!(matches!(
            dispatch_with_control(
                request,
                &ExecutionControl { cancellation: Some(cancellation), progress: None }
            ),
            Err(LifecycleError::Core(CoreError::Cancelled))
        ));
    }

    #[test]
    fn after_edit_carries_the_host_changed_path_into_fresh_context() {
        let mut request = AnalysisRequest::new(CommandDescriptor::context(PathBuf::from(".")));
        let mut map = MapSettings::default();
        map.task_seeds
            .changes
            .push(TaskChangeSeed::Path("src/parser.rs".to_owned()));
        request.map = map;
        request.context = Some(ContextRequest::default());

        let lifecycle =
            LifecycleRequest::new(LifecycleEvent::AfterEdit, request).expect("after edit accepts a context request");

        assert_eq!(
            lifecycle.request().map.task_seeds.changes,
            [TaskChangeSeed::Path("src/parser.rs".to_owned())]
        );
    }
}
