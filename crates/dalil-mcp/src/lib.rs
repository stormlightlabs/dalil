//! Model Context Protocol adapter for Dalil's typed, read-only analysis core.
//!
//! The stdio transport keeps protocol concerns out of `dalil-core`. Tool calls
//! return the same JSON reports as the CLI, so provenance, uncertainty,
//! omissions, and budget metadata retain their meanings across adapters.

use std::{path::PathBuf, sync::Arc};

use dalil_core::{
    AnalysisProfile, AnalysisRequest, CacheCommand, CacheMode, CancellationToken, ColorPolicy, CommandDescriptor,
    ContextRequest, ContextRevisionContext, CoreError, ExecutionControl, OutputFormat, SearchQueryMode, SearchRequest,
    SourceLanguage, TaskChangeSeed,
};
use rmcp::{
    RoleServer, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, JsonObject},
    service::RequestContext,
    tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Semaphore;

const MAX_ARGUMENT_ITEMS: usize = 64;
const MAX_ARGUMENT_TEXT_BYTES: usize = 4096;
const MAX_BUDGET: usize = 100_000;
const MAX_CONCURRENT_REQUESTS: usize = 4;
const SERVER_INSTRUCTIONS: &str = "Dalil provides bounded, read-only repository evidence. Read returned provenance, uncertainty, omissions, and budget fields before acting on recommendations.";

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct EmptyArguments {}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct CommonArguments {
    #[serde(default = "default_path")]
    path: String,
    task: Option<String>,
    symbols: Vec<String>,
    task_paths: Vec<String>,
    languages: Vec<SourceLanguage>,
    projects: Vec<String>,
    changed_paths: Vec<String>,
    changed_symbols: Vec<String>,
    search_terms: Vec<String>,
    focus: Vec<String>,
    focus_paths: Vec<String>,
    excludes: Vec<String>,
    budget: Option<usize>,
    profile: AnalysisProfile,
    cache: CacheMode,
    cache_files: Vec<String>,
    recursive: bool,
}

impl Default for CommonArguments {
    fn default() -> Self {
        Self {
            path: default_path(),
            task: None,
            symbols: Vec::new(),
            task_paths: Vec::new(),
            languages: Vec::new(),
            projects: Vec::new(),
            changed_paths: Vec::new(),
            changed_symbols: Vec::new(),
            search_terms: Vec::new(),
            focus: Vec::new(),
            focus_paths: Vec::new(),
            excludes: Vec::new(),
            budget: None,
            profile: AnalysisProfile::Compact,
            cache: CacheMode::Auto,
            cache_files: Vec::new(),
            recursive: false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExplainArguments {
    #[serde(flatten)]
    common: CommonArguments,
    target: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArguments {
    #[serde(flatten)]
    common: CommonArguments,
    query: Option<String>,
    symbol: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeArguments {
    #[serde(flatten)]
    common: CommonArguments,
    base: Option<String>,
    head: Option<String>,
    revision_range: Option<String>,
    dirty_worktree: bool,
    teach: bool,
}

struct DalilServer {
    requests: Arc<Semaphore>,
}

impl DalilServer {
    fn new() -> Self {
        Self { requests: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)) }
    }

    async fn execute(
        &self, request: Result<AnalysisRequest, String>, context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let request = match request {
            Ok(request) => request,
            Err(error) => return tool_error(&error),
        };
        let permit = match self.acquire_request() {
            Ok(permit) => permit,
            Err(result) => return result,
        };
        let cancellation = CancellationToken::default();
        let core_cancellation = cancellation.clone();
        let request_cancellation = context.ct.clone();
        let canceller = tokio::spawn(async move {
            request_cancellation.cancelled().await;
            core_cancellation.cancel();
        });
        let result = tokio::task::spawn_blocking(move || run_request(request, cancellation)).await;
        canceller.abort();
        drop(permit);

        match result {
            Ok(Ok(report)) => tool_result(report),
            Ok(Err(error)) => tool_error(&error),
            Err(error) => tool_error(&format!("analysis worker failed: {error}")),
        }
    }

    async fn execute_value(
        &self, operation: impl FnOnce() -> Result<Value, String> + Send + 'static,
    ) -> CallToolResult {
        let permit = match self.acquire_request() {
            Ok(permit) => permit,
            Err(result) => return result,
        };
        let result = tokio::task::spawn_blocking(operation).await;
        drop(permit);
        match result {
            Ok(Ok(value)) => tool_result(value),
            Ok(Err(error)) => tool_error(&error),
            Err(error) => tool_error(&format!("tool worker failed: {error}")),
        }
    }

    fn acquire_request(&self) -> Result<tokio::sync::OwnedSemaphorePermit, CallToolResult> {
        self.requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| tool_error("server is busy"))
    }
}

#[tool_router]
impl DalilServer {
    #[tool(
        name = "dalil_orient",
        description = "Start with a bounded repository briefing.",
        input_schema = common_schema(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false)
    )]
    async fn orient(
        &self, Parameters(arguments): Parameters<CommonArguments>, context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.execute(run_common(arguments, CommandDescriptor::orient, None, None), context)
            .await
    }

    #[tool(
        name = "dalil_map",
        description = "Return a bounded structural repository map.",
        input_schema = common_schema(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false)
    )]
    async fn map(
        &self, Parameters(arguments): Parameters<CommonArguments>, context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.execute(run_common(arguments, CommandDescriptor::map, None, None), context)
            .await
    }

    #[tool(
        name = "dalil_context",
        description = "Compile task-shaped repository context.",
        input_schema = change_schema(true),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false)
    )]
    async fn context(
        &self, Parameters(arguments): Parameters<ChangeArguments>, context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.execute(run_change(arguments, true), context).await
    }

    #[tool(
        name = "dalil_impact",
        description = "Inspect bounded evidence around local changes.",
        input_schema = change_schema(false),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false)
    )]
    async fn impact(
        &self, Parameters(arguments): Parameters<ChangeArguments>, context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.execute(run_change(arguments, false), context).await
    }

    #[tool(
        name = "dalil_explain",
        description = "Explain why a path or symbol is worth reading.",
        input_schema = explain_schema(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false)
    )]
    async fn explain(
        &self, Parameters(arguments): Parameters<ExplainArguments>, context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let request = (|| {
            validate_common(&arguments.common)?;
            validate_text(&arguments.target, "target")?;
            Ok(request_with_common(
                CommandDescriptor::explain(arguments.target, PathBuf::from(&arguments.common.path)),
                &arguments.common,
                None,
                None,
            ))
        })();
        self.execute(request, context).await
    }

    #[tool(
        name = "dalil_search",
        description = "Find bounded path, symbol, or concept anchors.",
        input_schema = search_schema(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false)
    )]
    async fn search(
        &self, Parameters(arguments): Parameters<SearchArguments>, context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.execute(run_search(arguments), context).await
    }

    #[tool(
        name = "dalil_capabilities",
        description = "Report installed Dalil language and limit capabilities.",
        input_schema = empty_schema(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false)
    )]
    async fn capabilities(&self, Parameters(_arguments): Parameters<EmptyArguments>) -> CallToolResult {
        self.execute_value(|| {
            serde_json::to_value(dalil_core::capabilities())
                .map_err(|error| format!("could not serialize capabilities: {error}"))
        })
        .await
    }

    #[tool(
        name = "dalil_cache_status",
        description = "Inspect Dalil's user-cache metadata without changing it.",
        input_schema = empty_schema(),
        annotations(read_only_hint = true, destructive_hint = false, open_world_hint = false)
    )]
    async fn cache_status(&self, Parameters(_arguments): Parameters<EmptyArguments>) -> CallToolResult {
        self.execute_value(|| {
            let report = dalil_core::cache(CacheCommand::Status).map_err(|error| error.to_string())?;
            serde_json::to_value(report).map_err(|error| format!("could not serialize cache status: {error}"))
        })
        .await
    }
}

#[rmcp::tool_handler(router = Self::tool_router())]
impl rmcp::ServerHandler for DalilServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo::new(rmcp::model::ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new("dalil", env!("CARGO_PKG_VERSION")))
            .with_instructions(SERVER_INSTRUCTIONS)
    }
}

fn default_path() -> String {
    ".".to_owned()
}

/// Serve the rmcp stdio transport until the client closes it.
pub async fn serve_stdio() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    DalilServer::new()
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}

fn run_common(
    arguments: CommonArguments, command: fn(PathBuf) -> CommandDescriptor, context: Option<ContextRequest>,
    search: Option<SearchRequest>,
) -> Result<AnalysisRequest, String> {
    validate_common(&arguments)?;
    Ok(request_with_common(
        command(PathBuf::from(&arguments.path)),
        &arguments,
        context,
        search,
    ))
}

fn run_search(arguments: SearchArguments) -> Result<AnalysisRequest, String> {
    validate_common(&arguments.common)?;
    if arguments.query.is_some() && arguments.symbol.is_some() {
        return Err("provide either `query` or `symbol`, not both".to_owned());
    }
    let (query, mode) = match (arguments.query, arguments.symbol) {
        (Some(query), None) => (query, SearchQueryMode::Plain),
        (None, Some(symbol)) => (symbol, SearchQueryMode::Symbol),
        (None, None) => return Err("provide `query` or `symbol`".to_owned()),
        (Some(_), Some(_)) => unreachable!(),
    };
    validate_text(&query, "query")?;
    let limit = arguments.limit.unwrap_or(5);
    if !(1..=MAX_ARGUMENT_ITEMS).contains(&limit) {
        return Err(format!("`limit` must be between 1 and {MAX_ARGUMENT_ITEMS}"));
    }
    let mut common = arguments.common;
    if mode == SearchQueryMode::Plain {
        common.task = Some(query.clone());
    } else {
        common.symbols.push(query.clone());
    }
    let budget = common.budget.unwrap_or(1_000);
    let search = SearchRequest {
        repository: common.path.clone(),
        query: query.clone(),
        mode,
        result_limit: limit,
        budget,
        profile: common.profile,
    };
    Ok(request_with_common(
        CommandDescriptor::search(query, PathBuf::from(&common.path)),
        &common,
        None,
        Some(search),
    ))
}

fn run_change(arguments: ChangeArguments, teaching_allowed: bool) -> Result<AnalysisRequest, String> {
    validate_common(&arguments.common)?;
    if arguments.revision_range.is_some() && (arguments.base.is_some() || arguments.head.is_some()) {
        return Err("`revision_range` cannot be combined with `base` or `head`".to_owned());
    }
    for (name, value) in [
        ("base", &arguments.base),
        ("head", &arguments.head),
        ("revision_range", &arguments.revision_range),
    ] {
        if let Some(value) = value {
            validate_text(value, name)?;
        }
    }
    if arguments.teach && !teaching_allowed {
        return Err("`teach` is only supported by `dalil_context`".to_owned());
    }
    let common = arguments.common;
    let budget = common.budget.unwrap_or(1_000);
    let context = ContextRequest {
        repository: common.path.clone(),
        task: common.task.clone(),
        symbols: common.symbols.clone(),
        paths: common.task_paths.clone(),
        projects: common.projects.clone(),
        changes: task_changes(&common),
        revision_context: ContextRevisionContext {
            base: arguments.base,
            head: arguments.head,
            range: arguments.revision_range,
            dirty_worktree: arguments.dirty_worktree,
        },
        change_resolution: Default::default(),
        budget,
        profile: common.profile,
        teaching: arguments.teach,
    };
    let command = if teaching_allowed {
        CommandDescriptor::context(PathBuf::from(&common.path))
    } else {
        CommandDescriptor::impact(PathBuf::from(&common.path))
    };
    Ok(request_with_common(command, &common, Some(context), None))
}

fn request_with_common(
    command: CommandDescriptor, common: &CommonArguments, context: Option<ContextRequest>,
    search: Option<SearchRequest>,
) -> AnalysisRequest {
    let mut request = AnalysisRequest::new(command);
    request.map.excludes = common.excludes.clone();
    request.map.focuses = common.focus.clone();
    request.map.focus_paths = common.focus_paths.clone();
    request.map.task_seeds.task = common.task.clone();
    request.map.task_seeds.symbols = common.symbols.clone();
    request.map.task_seeds.paths = common.task_paths.clone();
    request.map.task_seeds.languages = common.languages.clone();
    request.map.task_seeds.projects = common.projects.clone();
    request.map.task_seeds.changes = task_changes(common);
    request.map.task_seeds.search_terms = common.search_terms.clone();
    request.map.map_tokens = common.budget.unwrap_or(1_000);
    request.map.cache_mode = common.cache;
    request.map.cache_files = common.cache_files.clone();
    request.map.recursive = common.recursive;
    request.map.profile = common.profile;
    request.context = context;
    request.search = search;
    request.profile = common.profile;
    request.output_format = OutputFormat::Json;
    request.color_policy = ColorPolicy::Never;
    request
}

fn task_changes(common: &CommonArguments) -> Vec<TaskChangeSeed> {
    common
        .changed_paths
        .iter()
        .cloned()
        .map(TaskChangeSeed::Path)
        .chain(common.changed_symbols.iter().cloned().map(TaskChangeSeed::Symbol))
        .collect()
}

fn run_request(request: AnalysisRequest, cancellation: CancellationToken) -> Result<Value, String> {
    let report = dalil_core::analyze_with_control(
        request,
        &ExecutionControl { cancellation: Some(cancellation), progress: None },
    )
    .map_err(core_error)?;
    serde_json::to_value(report).map_err(|error| error.to_string())
}

fn core_error(error: CoreError) -> String {
    match error {
        CoreError::Cancelled => "request cancelled".to_owned(),
        other => other.to_string(),
    }
}

#[cfg(test)]
fn decode<T: for<'de> Deserialize<'de>>(arguments: Value) -> Result<T, String> {
    serde_json::from_value(arguments).map_err(|error| format!("invalid parameters: {error}"))
}

fn validate_common(arguments: &CommonArguments) -> Result<(), String> {
    validate_text(&arguments.path, "path")?;
    if let Some(task) = &arguments.task {
        validate_text(task, "task")?;
    }
    if let Some(budget) = arguments.budget {
        if !(1..=MAX_BUDGET).contains(&budget) {
            return Err(format!("`budget` must be between 1 and {MAX_BUDGET}"));
        }
    }
    for (name, values) in [
        ("symbols", &arguments.symbols),
        ("task_paths", &arguments.task_paths),
        ("projects", &arguments.projects),
        ("changed_paths", &arguments.changed_paths),
        ("changed_symbols", &arguments.changed_symbols),
        ("search_terms", &arguments.search_terms),
        ("focus", &arguments.focus),
        ("focus_paths", &arguments.focus_paths),
        ("excludes", &arguments.excludes),
        ("cache_files", &arguments.cache_files),
    ] {
        validate_text_list(values, name)?;
    }
    if arguments.languages.len() > MAX_ARGUMENT_ITEMS {
        return Err(format!("`languages` may contain at most {MAX_ARGUMENT_ITEMS} items"));
    }
    if arguments.cache == CacheMode::Files && arguments.cache_files.is_empty() {
        return Err("`cache: files` requires `cache_files`".to_owned());
    }
    Ok(())
}

fn validate_text_list(values: &[String], name: &str) -> Result<(), String> {
    if values.len() > MAX_ARGUMENT_ITEMS {
        return Err(format!("`{name}` may contain at most {MAX_ARGUMENT_ITEMS} items"));
    }
    for value in values {
        validate_text(value, name)?;
    }
    Ok(())
}

fn validate_text(value: &str, name: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("`{name}` must not be empty"));
    }
    if value.len() > MAX_ARGUMENT_TEXT_BYTES {
        return Err(format!("`{name}` may not exceed {MAX_ARGUMENT_TEXT_BYTES} bytes"));
    }
    Ok(())
}

fn tool_result(value: Value) -> CallToolResult {
    let serialized = match serde_json::to_vec(&value) {
        Ok(serialized) => serialized,
        Err(error) => return tool_error(&format!("could not serialize report: {error}")),
    };
    if let Some(limit) = value
        .get("limits")
        .and_then(|limits| limits.get("max_output_bytes"))
        .and_then(Value::as_u64)
    {
        if serialized.len() > limit as usize {
            return tool_error(&format!(
                "report exceeds Dalil's {limit}-byte output limit; use the compact profile or a narrower scope"
            ));
        }
    }
    let summary = value
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| value.get("report_kind").and_then(Value::as_str))
        .unwrap_or("Dalil report")
        .to_owned();
    let mut result = CallToolResult::structured(value);
    result.content = vec![ContentBlock::text(summary)];
    result
}

fn tool_error(message: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message)])
}

fn empty_schema() -> Arc<JsonObject> {
    schema(json!({ "type": "object", "additionalProperties": false }))
}

fn common_schema() -> Arc<JsonObject> {
    schema(json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "path": { "type": "string", "description": "Repository or subdirectory. Defaults to the current directory." },
            "task": { "type": "string" },
            "symbols": string_array_schema(),
            "task_paths": string_array_schema(),
            "languages": { "type": "array", "items": { "enum": ["rust", "javascript", "javascript_jsx", "typescript", "typescript_tsx", "python", "ruby", "java", "c_sharp", "go", "lua", "zig"] }, "maxItems": MAX_ARGUMENT_ITEMS },
            "projects": string_array_schema(),
            "changed_paths": string_array_schema(),
            "changed_symbols": string_array_schema(),
            "search_terms": string_array_schema(),
            "focus": string_array_schema(),
            "focus_paths": string_array_schema(),
            "excludes": string_array_schema(),
            "budget": { "type": "integer", "minimum": 1, "maximum": MAX_BUDGET },
            "profile": { "enum": ["compact", "evidence"] },
            "cache": { "enum": ["auto", "always", "files", "manual", "disabled"] },
            "cache_files": string_array_schema(),
            "recursive": { "type": "boolean" }
        }
    }))
}

fn change_schema(teaching: bool) -> Arc<JsonObject> {
    let mut schema = common_schema();
    let properties = Arc::make_mut(&mut schema)
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("common schema has properties");
    properties.insert("base".to_owned(), json!({ "type": "string" }));
    properties.insert("head".to_owned(), json!({ "type": "string" }));
    properties.insert(
        "revision_range".to_owned(),
        json!({ "type": "string", "description": "One local base..head range." }),
    );
    properties.insert("dirty_worktree".to_owned(), json!({ "type": "boolean" }));
    if teaching {
        properties.insert("teach".to_owned(), json!({ "type": "boolean" }));
    }
    schema
}

fn explain_schema() -> Arc<JsonObject> {
    let mut schema = common_schema();
    Arc::make_mut(&mut schema)
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("common schema has properties")
        .insert(
            "target".to_owned(),
            json!({ "type": "string", "description": "Path or symbol to explain." }),
        );
    Arc::make_mut(&mut schema).insert("required".to_owned(), json!(["target"]));
    schema
}

fn search_schema() -> Arc<JsonObject> {
    let mut schema = common_schema();
    let properties = Arc::make_mut(&mut schema)
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("common schema has properties");
    properties.insert(
        "query".to_owned(),
        json!({ "type": "string", "description": "Path or concept query." }),
    );
    properties.insert(
        "symbol".to_owned(),
        json!({ "type": "string", "description": "Exact symbol query." }),
    );
    properties.insert(
        "limit".to_owned(),
        json!({ "type": "integer", "minimum": 1, "maximum": MAX_ARGUMENT_ITEMS }),
    );
    schema
}

fn string_array_schema() -> Value {
    json!({ "type": "array", "items": { "type": "string", "maxLength": MAX_ARGUMENT_TEXT_BYTES }, "maxItems": MAX_ARGUMENT_ITEMS })
}

fn schema(value: Value) -> Arc<JsonObject> {
    let Value::Object(value) = value else {
        unreachable!("MCP tool schemas must be JSON objects");
    };
    Arc::new(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::{ServerHandler, model::ToolAnnotations};

    #[test]
    fn tools_are_read_only_and_bounded() {
        let tools = DalilServer::tool_router().list_all();
        assert_eq!(tools.len(), 8);
        assert!(tools.iter().all(|tool| {
            tool.annotations
                == Some(ToolAnnotations::from_raw(
                    None,
                    Some(true),
                    Some(false),
                    None,
                    Some(false),
                ))
        }));
        assert!(tools.iter().all(|tool| tool.name != "dalil_graph"));
        assert_eq!(common_schema()["properties"]["budget"]["maximum"], MAX_BUDGET);
    }

    #[test]
    fn server_info_and_tools_come_from_rmcp() {
        let server = DalilServer::new();
        let info = server.get_info();
        assert_eq!(info.server_info.name, "dalil");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.instructions.as_deref(), Some(SERVER_INSTRUCTIONS));
        assert!(info.capabilities.tools.is_some());
    }

    #[test]
    fn tool_errors_map_invalid_and_oversized_parameters() {
        let oversized = json!({ "path": "x".repeat(MAX_ARGUMENT_TEXT_BYTES + 1) });
        let oversized: CommonArguments = decode(oversized).expect("arguments deserialize");
        let unknown = decode::<CommonArguments>(json!({ "unexpected": true }));

        assert!(validate_common(&oversized).is_err());
        assert!(unknown.is_err());
    }

    #[test]
    fn cancelled_analysis_maps_to_a_tool_error() {
        let token = CancellationToken::default();
        token.cancel();
        let request = run_common(CommonArguments::default(), CommandDescriptor::map, None, None)
            .expect("default map request is valid");
        let response = run_request(request, token).map_err(|error| tool_error(&error));
        assert_eq!(
            response.expect_err("cancelled request is an error").content[0]
                .as_text()
                .map(|text| text.text.as_str()),
            Some("request cancelled")
        );
    }

    #[test]
    fn mcp_map_matches_the_shared_core_operation() {
        let root = std::fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("workspace root exists");
        let arguments = json!({
            "path": root,
            "budget": 500,
            "cache": "disabled"
        });
        let common: CommonArguments = decode(arguments).expect("MCP arguments remain valid");
        let actual = run_request(
            run_common(common.clone(), CommandDescriptor::map, None, None).expect("map request is valid"),
            CancellationToken::default(),
        )
        .expect("core map succeeds");
        let expected = run_request(
            request_with_common(CommandDescriptor::map(PathBuf::from(&common.path)), &common, None, None),
            CancellationToken::default(),
        )
        .expect("core map succeeds");

        assert_eq!(actual, expected);
    }
}
