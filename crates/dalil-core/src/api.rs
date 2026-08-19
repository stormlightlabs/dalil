use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serde::{Deserialize, Serialize};

use crate::{
    AnalysisProfile, CacheCommand, CacheControlReport, CommandDescriptor, CommandName, ContextBundle, ContextRequest,
    ExplainReport, HistorySettings, ImpactReport, MapReport, MapSettings, OrientationReport, QueryRequest,
    QueryResults, RelationshipRequest, RelationshipResults, Report, ReportError, SearchRequest, SearchResults,
};

/// The report serialization selected by an adapter. Rendering is implemented outside the core.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    #[default]
    Markdown,
    Json,
    Html,
}

/// Diagnostic color policy retained in report provenance for compatibility.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorPolicy {
    #[default]
    Auto,
    Always,
    Never,
}

/// Stable categories for adapter error handling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitCategory {
    Success,
    Usage,
    Repository,
    Input,
    Analysis,
    Internal,
}

impl ExitCategory {
    pub const fn code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Usage => 2,
            Self::Repository => 3,
            Self::Input => 4,
            Self::Analysis => 5,
            Self::Internal => 70,
        }
    }
}

/// A complete typed request for one repository-analysis operation.
#[derive(Debug)]
pub struct AnalysisRequest {
    pub command: CommandDescriptor,
    pub history: HistorySettings,
    pub map: MapSettings,
    pub context: Option<ContextRequest>,
    pub search: Option<SearchRequest>,
    pub profile: AnalysisProfile,
    pub output_format: OutputFormat,
    pub color_policy: ColorPolicy,
    pub strict: bool,
}

impl AnalysisRequest {
    pub fn new(command: CommandDescriptor) -> Self {
        Self {
            command,
            history: HistorySettings::default(),
            map: MapSettings::default(),
            context: None,
            search: None,
            profile: AnalysisProfile::Compact,
            output_format: OutputFormat::Markdown,
            color_policy: ColorPolicy::Never,
            strict: false,
        }
    }
}

/// A cooperative cancellation handle. Cancellation is observed at operation
/// boundaries; callers should run each request in its own task if they need to
/// stop underlying filesystem or Git reads immediately.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Progress events emitted at stable operation boundaries. Events never affect
/// selected evidence, budgets, warnings, or result ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressPhase {
    Started,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub command: CommandName,
    pub phase: ProgressPhase,
}

pub trait ProgressObserver: Send + Sync {
    fn on_progress(&self, event: ProgressEvent);
}

/// Optional execution controls for a single synchronous operation.
#[derive(Default)]
pub struct ExecutionControl {
    pub cancellation: Option<CancellationToken>,
    pub progress: Option<Arc<dyn ProgressObserver>>,
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("analysis was cancelled")]
    Cancelled,
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error("request command must be `{expected}`, received `{actual}`")]
    WrongOperation {
        expected: &'static str,
        actual: &'static str,
    },
}

/// Execute an operation and return its full typed report envelope.
pub fn analyze(request: AnalysisRequest) -> Result<Report, CoreError> {
    analyze_with_control(request, &ExecutionControl::default())
}

/// Execute an operation with cooperative cancellation and boundary progress.
pub fn analyze_with_control(request: AnalysisRequest, control: &ExecutionControl) -> Result<Report, CoreError> {
    if control
        .cancellation
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
    {
        return Err(CoreError::Cancelled);
    }
    let command = request.command.name;
    if let Some(observer) = &control.progress {
        observer.on_progress(ProgressEvent { command, phase: ProgressPhase::Started });
    }
    let report = Report::analyze(request)?;
    if control
        .cancellation
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
    {
        return Err(CoreError::Cancelled);
    }
    if let Some(observer) = &control.progress {
        observer.on_progress(ProgressEvent { command, phase: ProgressPhase::Completed });
    }
    Ok(report)
}

/// Run orientation and return its specialized typed result.
pub fn orient(request: AnalysisRequest) -> Result<OrientationReport, CoreError> {
    specialized(request, CommandName::Orient, "orient", |report| report.orientation)
}

/// Run repository mapping and return its specialized typed result.
pub fn map(request: AnalysisRequest) -> Result<MapReport, CoreError> {
    specialized(request, CommandName::Map, "map", |report| report.map)
}

/// Compile a task-shaped context bundle.
pub fn context(request: AnalysisRequest) -> Result<ContextBundle, CoreError> {
    specialized(request, CommandName::Context, "context", |report| report.context)
}

/// Compile impact context for local changes.
pub fn impact(request: AnalysisRequest) -> Result<ImpactReport, CoreError> {
    specialized(request, CommandName::Impact, "impact", |report| report.impact)
}

/// Explain a selected path or symbol recommendation.
pub fn explain(request: AnalysisRequest) -> Result<ExplainReport, CoreError> {
    specialized(request, CommandName::Explain, "explain", |report| report.explain)
}

/// Find path, symbol, or concept anchors.
pub fn search(request: AnalysisRequest) -> Result<SearchResults, CoreError> {
    specialized(request, CommandName::Search, "search", |report| report.search)
}

/// Run a typed repository query without requiring an adapter to reconstruct
/// repository facts from a rendered report.
pub fn query(request: QueryRequest) -> Result<QueryResults, CoreError> {
    let revision = request.revision.clone();
    let settings = MapSettings {
        profile: request.profile,
        map_tokens: request.budget.max(1),
        cache_mode: request.cache_mode,
        ..MapSettings::default()
    };
    let map =
        crate::map::analyze_with_history(Path::new(&request.repository), &settings, None).map_err(ReportError::Map)?;
    let changes = if revision.is_requested() {
        let context: crate::ContextRevisionContext = (&revision).into();
        crate::map::resolve_changes(Path::new(&request.repository), &context).map_err(ReportError::Map)?
    } else {
        crate::ChangeResolution::default()
    };
    Ok(crate::report::compile_query(request, &map, changes))
}

/// Run a paged symbol or relationship operation over the repository graph.
pub fn relationships(request: RelationshipRequest) -> Result<RelationshipResults, CoreError> {
    let settings = MapSettings {
        profile: request.profile,
        map_tokens: request.budget.max(1),
        cache_mode: request.cache_mode,
        ..MapSettings::default()
    };
    let map =
        crate::map::analyze_with_history(Path::new(&request.repository), &settings, None).map_err(ReportError::Map)?;
    Ok(crate::report::compile_relationships(request, &map))
}

/// Return installed analysis capabilities without opening a repository.
pub fn capabilities() -> crate::CapabilitiesReport {
    crate::CapabilitiesReport::current()
}

/// Inspect or maintain the user cache without analyzing source.
pub fn cache(command: CacheCommand) -> Result<CacheControlReport, crate::MapError> {
    crate::cache_control(command)
}

fn specialized<T>(
    request: AnalysisRequest, expected: CommandName, expected_label: &'static str,
    select: impl FnOnce(Report) -> Option<T>,
) -> Result<T, CoreError> {
    let actual = request.command.name;
    if actual != expected {
        return Err(CoreError::WrongOperation { expected: expected_label, actual: actual.label() });
    }
    select(analyze(request)?).ok_or(CoreError::WrongOperation { expected: expected_label, actual: expected_label })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn typed_query_runs_at_the_core_boundary() {
        let mut request = QueryRequest::new(env!("CARGO_MANIFEST_DIR"));
        request.cache_mode = crate::CacheMode::Disabled;
        let results = query(request.clone()).expect("typed query succeeds");
        let repeated = query(request).expect("repeated typed query succeeds");
        assert_eq!(
            serde_json::to_value(&results).expect("query serializes"),
            serde_json::to_value(repeated).expect("repeated query serializes")
        );
        assert!(results.bounds.total > 0);
        assert!(!results.matches.is_empty());
        assert!(results.provenance.query_packs.contains_key("rust"));
    }

    #[test]
    fn relationship_query_returns_stable_typed_nodes_and_edges() {
        let mut request = RelationshipRequest::new(
            env!("CARGO_MANIFEST_DIR"),
            crate::RelationshipOperation::Definitions,
            "MapReport",
        );
        request.cache_mode = crate::CacheMode::Disabled;
        let results = relationships(request.clone()).expect("relationship query succeeds");
        let repeated = relationships(request).expect("repeated relationship query succeeds");
        assert_eq!(
            serde_json::to_value(&results).expect("relationship serializes"),
            serde_json::to_value(repeated).expect("repeated relationship serializes")
        );
        assert!(!results.matches.is_empty());
        assert!(results.matches.iter().all(|item| item.node.id.starts_with("symbol:")));
        assert!(results.provenance.symbols.total > 0);
    }

    #[test]
    fn cancelled_request_returns_before_repository_access() {
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        let control = ExecutionControl { cancellation: Some(cancellation), progress: None };
        let request = AnalysisRequest::new(CommandDescriptor::map(PathBuf::from("not-a-repository")));

        assert!(matches!(
            analyze_with_control(request, &control),
            Err(CoreError::Cancelled)
        ));
    }
}
