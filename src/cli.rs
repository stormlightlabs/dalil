use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use clap::{
    ArgAction, ColorChoice, Command, CommandFactory, Parser, Subcommand, ValueEnum, builder::Styles, error::ErrorKind,
};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};

use crate::map::{CacheCommand, CacheControlReport, MapSettings};
use crate::report::{
    AnalysisProfile, CacheMode, CapabilitiesReport, CommandDescriptor, ContextRequest, ContextRevisionContext,
    DoctorReport, HistoryOperation, HistorySettings, KeywordMatchMode, Report, SourceLanguage, StrictIssue,
    TaskChangeSeed, TaskSeeds,
};
use crate::utils;

#[derive(Debug, Subcommand)]
enum SubcommandName {
    /// Give a concise answer about where to start in a repository.
    Orient(OrientCommand),
    /// Produce only the structural repository map.
    Map(MapCommand),
    /// Compile one bounded, task-oriented context bundle.
    Context(ContextCommand),
    /// Inspect bounded evidence surrounding a local revision range or dirty worktree.
    Impact(ImpactCommand),
    /// Explain the bounded evidence behind a path or symbol recommendation.
    Explain(ExplainCommand),
    /// Produce Git-history findings, or select one focused history signal.
    History(HistoryCommand),
    /// Inspect or control retained source-analysis cache data.
    Cache(CacheCommandCli),
    /// Report installed schema, language, query-pack, and limit capabilities.
    Capabilities,
    /// Check local discovery and Dalil support without analyzing source.
    Doctor(DoctorCommand),
}

#[derive(Clone, Copy, Debug, clap::Subcommand)]
enum CacheOperation {
    /// Print the configured cache root.
    Path,
    /// Report cache record count, size, repositories, and retention limits.
    Status,
    /// Remove expired and over-limit records.
    Prune,
    /// Remove all Dalil cache records.
    Clear,
}

impl From<CacheOperation> for CacheCommand {
    fn from(operation: CacheOperation) -> Self {
        match operation {
            CacheOperation::Path => Self::Path,
            CacheOperation::Status => Self::Status,
            CacheOperation::Prune => Self::Prune,
            CacheOperation::Clear => Self::Clear,
        }
    }
}

#[derive(Debug, Subcommand)]
enum HistorySubcommand {
    /// Show changed-path frequency over the configured time window.
    Churn(HistoryOperationCommand),
    /// Show commit-author concentration.
    Contributors(HistoryOperationCommand),
    /// Show fix-related path clusters and churn overlap.
    Bugs(HistoryOperationCommand),
    /// Show author-date activity grouped by month.
    Activity(HistoryOperationCommand),
    /// Show commits using firefighting language.
    Firefighting(HistoryOperationCommand),
}

/// The report serialization selected by `--format`, `--json`, or `--html`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    #[default]
    Markdown,
    Json,
    Html,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CacheModeOption {
    Auto,
    Always,
    Files,
    Manual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ProfileOption {
    Compact,
    Evidence,
}

impl From<ProfileOption> for AnalysisProfile {
    fn from(profile: ProfileOption) -> Self {
        match profile {
            ProfileOption::Compact => Self::Compact,
            ProfileOption::Evidence => Self::Evidence,
        }
    }
}

#[derive(Debug, clap::Args)]
struct DoctorCommand {
    #[arg(
        value_name = "PATH",
        default_value = ".",
        help = "Repository or subdirectory to inspect (default: current directory)."
    )]
    path: PathBuf,
}

impl From<CacheModeOption> for CacheMode {
    fn from(mode: CacheModeOption) -> Self {
        match mode {
            CacheModeOption::Auto => Self::Auto,
            CacheModeOption::Always => Self::Always,
            CacheModeOption::Files => Self::Files,
            CacheModeOption::Manual => Self::Manual,
        }
    }
}

/// The diagnostic color policy selected by `--color` or `--no-color`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ColorPolicy {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorPolicy {
    fn should_color(self, is_terminal: bool, environment: ColorEnvironment) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => is_terminal && !environment.no_color && !environment.term_is_dumb,
        }
    }
}

/// Stable categories for command termination.
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
    /// The process status documented in command help.
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

#[derive(Debug, thiserror::Error)]
enum ApplicationError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Report(#[source] crate::report::ReportError),
    #[error("strict report policy rejected: {issues:?}")]
    Strict { issues: Vec<StrictIssue> },
    #[error("doctor found one or more failing checks")]
    DoctorFailed,
    #[error("could not open HTML report `{path}`")]
    OpenReport {
        path: PathBuf,
        #[source]
        error: io::Error,
    },
}

impl From<ApplicationError> for ExitCategory {
    fn from(error: ApplicationError) -> Self {
        match error {
            ApplicationError::Usage(_) => ExitCategory::Usage,
            ApplicationError::Report(error) => match error {
                crate::report::ReportError::History(error) => error.into(),
                crate::report::ReportError::Map(error) => error.into(),
                crate::report::ReportError::Json(_)
                | crate::report::ReportError::Html(_)
                | crate::report::ReportError::OutputLimit(_) => ExitCategory::Internal,
            },
            ApplicationError::Strict { .. } => ExitCategory::Analysis,
            ApplicationError::DoctorFailed => ExitCategory::Repository,
            ApplicationError::OpenReport { .. } => ExitCategory::Internal,
        }
    }
}

impl From<&ApplicationError> for ExitCategory {
    fn from(value: &ApplicationError) -> Self {
        match value {
            ApplicationError::Usage(_) => ExitCategory::Usage,
            ApplicationError::Report(error) => match error {
                crate::report::ReportError::History(error) => error.into(),
                crate::report::ReportError::Map(error) => error.into(),
                crate::report::ReportError::Json(_)
                | crate::report::ReportError::Html(_)
                | crate::report::ReportError::OutputLimit(_) => ExitCategory::Internal,
            },
            ApplicationError::Strict { .. } => ExitCategory::Analysis,
            ApplicationError::DoctorFailed => ExitCategory::Repository,
            ApplicationError::OpenReport { .. } => ExitCategory::Internal,
        }
    }
}

impl ApplicationError {
    fn usage(message: impl Into<String>) -> Self {
        Self::Usage(message.into())
    }
}

#[derive(Debug, clap::Args)]
struct CacheCommandCli {
    #[command(subcommand)]
    operation: CacheOperation,
}

#[derive(Debug, Parser)]
#[command(
    name = "dalil",
    version,
    about = "Read-only repository orientation for people and coding agents.",
    long_about = "Dalil produces a concise, evidence-backed repository orientation.

The default command and `orient` return the same concise orientation report.

Primary workflows: `orient`, `map`, `context`, `impact`, `search`, and `explain`.
`search` is not available in this release. Use `history` for focused evidence
inspection. Use `cache`, `capabilities`, and `doctor` for maintenance.

Examples:
    dalil .
    dalil orient .
    dalil --json .
    dalil orient --json .
    dalil --html . > dalil-report.html
    dalil --html --open .
    dalil --focus parser --focus-path src .
    dalil --no-cache .
    dalil map --json
    dalil history contributors .
    dalil explain src/parser.rs --json
    dalil context --task 'inspect parser cache' --json
    dalil impact --revision-range 'HEAD~1..HEAD' --json
    dalil capabilities --json
    dalil doctor --json

See https://github.com/stormlightlabs/dalil/issues for support and bug reports.

Exit status:
    0  success
    2  command-line usage error
    3  repository discovery failure
    4  input or access failure
    5  analysis failure
    70  internal failure
",
    color = ColorChoice::Never,
    styles = Styles::plain(),
    disable_help_subcommand = true
)]
struct Cli {
    #[command(flatten)]
    output: OutputOptions,

    #[command(flatten)]
    map_options: MapOptions,

    #[command(subcommand)]
    command: Option<SubcommandName>,

    #[arg(
        value_name = "PATH",
        default_value = ".",
        help = "Repository or subdirectory to analyze (default: current directory)."
    )]
    path: PathBuf,
}

/// Build the authoritative Clap command used by help, completions, and man pages.
pub fn command() -> Command {
    Cli::command()
}

impl From<Cli> for CommandRequest {
    fn from(cli: Cli) -> Self {
        let output_format = cli.output.format.unwrap_or(if cli.output.json {
            OutputFormat::Json
        } else if cli.output.html {
            OutputFormat::Html
        } else {
            OutputFormat::Markdown
        });
        let color_policy = cli.color_policy();
        let strict = cli.output.strict;
        let profile = cli.output.profile.into();
        let default_map_settings = cli.map_options.settings();
        let (command, history, map_settings, context) = match cli.command {
            None => (
                CommandDescriptor::orient(cli.path),
                HistorySettings::default(),
                default_map_settings,
                None,
            ),
            Some(SubcommandName::Orient(orient)) => {
                let OrientCommand { options, path } = orient;
                (
                    CommandDescriptor::orient(path),
                    HistorySettings::default(),
                    options.settings(),
                    None,
                )
            }
            Some(SubcommandName::Map(map)) => {
                let MapCommand { options, path } = map;
                (
                    CommandDescriptor::map(path),
                    HistorySettings::default(),
                    options.settings(),
                    None,
                )
            }
            Some(SubcommandName::History(history)) => {
                let inherited = history.options.settings();
                match history.operation {
                    Some(operation) => {
                        let (operation, path, settings) = operation.into_parts(&inherited);
                        (
                            CommandDescriptor::history(path, Some(operation)),
                            settings,
                            MapSettings::default(),
                            None,
                        )
                    }
                    None => (
                        CommandDescriptor::history(history.path, None),
                        inherited,
                        MapSettings::default(),
                        None,
                    ),
                }
            }
            Some(SubcommandName::Cache(_)) | Some(SubcommandName::Capabilities) | Some(SubcommandName::Doctor(_)) => (
                CommandDescriptor::map(cli.path),
                HistorySettings::default(),
                default_map_settings,
                None,
            ),
            Some(SubcommandName::Explain(explain)) => (
                CommandDescriptor::explain(explain.target, explain.path),
                HistorySettings::default(),
                explain.options.settings(),
                None,
            ),
            Some(SubcommandName::Context(context)) => {
                let ContextCommand { options, revision, teach, path } = context;
                let map = options.settings();
                let request = change_context_request(&path, &map, revision, profile, teach);
                (
                    CommandDescriptor::context(path),
                    HistorySettings::default(),
                    map,
                    Some(request),
                )
            }
            Some(SubcommandName::Impact(impact)) => {
                let ImpactCommand { options, revision, path } = impact;
                let map = options.settings();
                let request = change_context_request(&path, &map, revision, profile, false);
                (
                    CommandDescriptor::impact(path),
                    HistorySettings::default(),
                    map,
                    Some(request),
                )
            }
        };

        let mut map = map_settings;
        map.profile = profile;
        CommandRequest { command, history, map, context, profile, output_format, color_policy, strict }
    }
}

impl Cli {
    fn output_format(&self) -> Result<OutputFormat, ApplicationError> {
        self.output.format()
    }

    fn color_policy(&self) -> ColorPolicy {
        self.output.color_policy()
    }

    fn validate(&self) -> Result<(), ApplicationError> {
        self.map_options.validate()?;
        if let Some(SubcommandName::Orient(orient)) = &self.command {
            orient.options.validate()?;
        }
        if let Some(SubcommandName::Map(map)) = &self.command {
            map.options.validate()?;
        }
        if let Some(SubcommandName::Explain(explain)) = &self.command {
            explain.options.validate()?;
            if explain.target.trim().is_empty() {
                return Err(ApplicationError::usage(
                    "`explain` requires a non-empty path or symbol target",
                ));
            }
        }
        if let Some(SubcommandName::Context(context)) = &self.command {
            context.options.validate()?;
            context.revision.validate()?;
        }
        if let Some(SubcommandName::Impact(impact)) = &self.command {
            impact.options.validate()?;
            impact.revision.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, clap::Args)]
#[command(after_help = "Examples:
    dalil orient
    dalil orient --json
    dalil orient --task 'inspect parser cache'

The default `dalil` command runs this same orientation workflow.

Support: https://github.com/stormlightlabs/dalil/issues
")]
struct OrientCommand {
    #[command(flatten)]
    options: MapOptions,

    #[arg(
        value_name = "PATH",
        default_value = ".",
        help = "Repository or subdirectory to orient in (default: current directory)."
    )]
    path: PathBuf,
}

#[derive(Debug, clap::Args)]
#[command(after_help = "Examples:
    dalil map
    dalil map --json
    dalil map --html > dalil-map.html

Support: https://github.com/stormlightlabs/dalil/issues
")]
struct MapCommand {
    #[command(flatten)]
    options: MapOptions,

    #[arg(
        value_name = "PATH",
        default_value = ".",
        help = "Repository or subdirectory to analyze (default: current directory)."
    )]
    path: PathBuf,
}

#[derive(Debug, clap::Args)]
#[command(after_help = "Examples:
    dalil explain src/parser.rs --json
    dalil explain Parser --focus Parser --json

The explanation gives per-path reading guidance with ranking contributions,
relationships, relevant history, uncertainty, and a suggested next read. It is
heuristic evidence, not a semantic call graph.

Support: https://github.com/stormlightlabs/dalil/issues
")]
struct ExplainCommand {
    #[command(flatten)]
    options: MapOptions,

    #[arg(value_name = "PATH-OR-SYMBOL", help = "Path or symbol to explain.")]
    target: String,

    #[arg(
        value_name = "PATH",
        default_value = ".",
        help = "Repository or subdirectory to analyze."
    )]
    path: PathBuf,
}

#[derive(Debug, clap::Args)]
#[command(after_help = "Examples:
    dalil context --task 'fix parser cache invalidation'
    dalil context --task 'review cache changes' --changed-path src/map/cache.rs --symbol CacheStore --json

Context combines orientation, selected files and symbols, lexical relationships,
likely tests, bounded history, uncertainty, omissions, and next reads under one
budget. Add `--teach` for a source-grounded reading sequence. Revision ranges
and `--dirty-worktree` resolve local changes without running repository code.

Support: https://github.com/stormlightlabs/dalil/issues
")]
struct ContextCommand {
    #[command(flatten)]
    options: MapOptions,

    #[command(flatten)]
    revision: ContextRevisionOptions,

    /// Add a source-grounded teaching sequence to the selected context evidence.
    #[arg(long, visible_alias = "teaching", action = ArgAction::SetTrue)]
    teach: bool,

    #[arg(
        value_name = "PATH",
        default_value = ".",
        help = "Repository or subdirectory to analyze (default: current directory)."
    )]
    path: PathBuf,
}

#[derive(Debug, clap::Args)]
#[command(after_help = "Examples:
    dalil impact --revision-range 'HEAD~1..HEAD'
    dalil impact --dirty-worktree --task 'review parser changes' --json

Impact reports changed symbols, related inspection targets, likely tests, ownership configuration, and history evidence. It labels lexical, structural, manifest, and history relationships without claiming that a change will break code.

Support: https://github.com/stormlightlabs/dalil/issues
")]
struct ImpactCommand {
    #[command(flatten)]
    options: MapOptions,

    #[command(flatten)]
    revision: ContextRevisionOptions,

    #[arg(
        value_name = "PATH",
        default_value = ".",
        help = "Repository or subdirectory to analyze (default: current directory)."
    )]
    path: PathBuf,
}

fn change_context_request(
    path: &Path, map: &MapSettings, revision: ContextRevisionOptions, profile: AnalysisProfile, teaching: bool,
) -> ContextRequest {
    ContextRequest {
        repository: path.to_string_lossy().into_owned(),
        task: map.task_seeds.task.clone(),
        symbols: map.task_seeds.symbols.clone(),
        paths: map.task_seeds.paths.clone(),
        projects: map.task_seeds.projects.clone(),
        changes: map.task_seeds.changes.clone(),
        revision_context: revision.into(),
        change_resolution: Default::default(),
        budget: map.map_tokens,
        profile,
        teaching,
    }
}

#[derive(Clone, Debug, Default, clap::Args)]
struct ContextRevisionOptions {
    /// Compare this local base revision with `--head` (or `HEAD` when omitted).
    #[arg(long, value_name = "REVISION")]
    base: Option<String>,

    /// Compare `--base` (or `HEAD` when omitted) with this local head revision.
    #[arg(long, value_name = "REVISION")]
    head: Option<String>,

    /// Resolve one local `base..head` revision range into changed paths and symbols.
    #[arg(long = "revision-range", value_name = "RANGE", conflicts_with_all = ["base", "head"])]
    range: Option<String>,

    /// Resolve modified, deleted, and untracked paths from the local dirty worktree.
    #[arg(long = "dirty-worktree", action = ArgAction::SetTrue)]
    dirty_worktree: bool,
}

impl ContextRevisionOptions {
    fn validate(&self) -> Result<(), ApplicationError> {
        for (name, value) in [
            ("--base", &self.base),
            ("--head", &self.head),
            ("--revision-range", &self.range),
        ] {
            if value.as_ref().is_some_and(|value| value.trim().is_empty()) {
                return Err(ApplicationError::usage(format!("`{name}` must not be empty")));
            }
        }
        Ok(())
    }
}

impl From<ContextRevisionOptions> for ContextRevisionContext {
    fn from(options: ContextRevisionOptions) -> Self {
        Self {
            base: options.base.map(|value| value.trim().to_owned()),
            head: options.head.map(|value| value.trim().to_owned()),
            range: options.range.map(|value| value.trim().to_owned()),
            dirty_worktree: options.dirty_worktree,
        }
    }
}

#[derive(Clone, Debug, clap::Args)]
struct MapOptions {
    /// Exclude paths from analysis using a Git-style glob; repeat for multiple exclusions.
    #[arg(long = "exclude", value_name = "GLOB", action = ArgAction::Append)]
    excludes: Vec<String>,

    /// Boost symbols and paths matching this explicit task text; repeat for multiple terms.
    #[arg(long = "focus", value_name = "TEXT", action = ArgAction::Append)]
    focuses: Vec<String>,

    /// Boost files under this explicit path; repeat for multiple paths.
    #[arg(long = "focus-path", value_name = "PATH", action = ArgAction::Append)]
    focus_paths: Vec<String>,

    /// Describe the task so Dalil can derive deterministic lexical ranking seeds.
    #[arg(long, value_name = "TEXT")]
    task: Option<String>,

    /// Rank files defining or referring to this symbol; repeat for multiple symbols.
    #[arg(long = "symbol", value_name = "NAME", action = ArgAction::Append)]
    symbols: Vec<String>,

    /// Rank files under this task-relevant path; repeat for multiple paths.
    #[arg(long = "task-path", value_name = "PATH", action = ArgAction::Append)]
    task_paths: Vec<String>,

    /// Rank files in this source language; repeat for multiple languages.
    #[arg(long = "language", value_name = "LANGUAGE", action = ArgAction::Append, value_parser = parse_source_language)]
    languages: Vec<SourceLanguage>,

    /// Rank files belonging to this project root; repeat for multiple roots.
    #[arg(long = "project", value_name = "PATH", action = ArgAction::Append)]
    projects: Vec<String>,

    /// Rank files under this changed path; repeat for multiple changed paths.
    #[arg(long = "changed-path", value_name = "PATH", action = ArgAction::Append)]
    changed_paths: Vec<String>,

    /// Rank files defining or referring to this changed symbol; repeat for multiple symbols.
    #[arg(long = "changed-symbol", value_name = "NAME", action = ArgAction::Append)]
    changed_symbols: Vec<String>,

    /// Add a concise lexical task term; repeat for multiple terms.
    #[arg(long = "search", value_name = "TERM", action = ArgAction::Append)]
    search_terms: Vec<String>,

    /// Maximum estimated tokens in the compact report and ranked structural evidence (default: 1000).
    #[arg(long = "budget", value_name = "N", default_value_t = 1_000, value_parser = clap::value_parser!(usize))]
    budget: usize,

    /// Cache policy: auto, always, files, or manual (default: auto).
    #[arg(
        long = "cache",
        visible_alias = "cache-mode",
        value_name = "MODE",
        value_enum,
        default_value_t = CacheModeOption::Auto
    )]
    cache_mode: CacheModeOption,

    /// Refresh only these paths when `--cache files` is selected; repeat as needed.
    #[arg(long = "cache-file", visible_alias = "changed-file", value_name = "PATH", action = ArgAction::Append)]
    cache_files: Vec<String>,

    /// Disable all cache reads and writes.
    #[arg(long = "no-cache", action = ArgAction::SetTrue)]
    no_cache: bool,

    /// Descend into nested repositories and checked-out submodules.
    #[arg(long = "recursive", action = ArgAction::SetTrue)]
    recursive: bool,
}

impl From<MapOptions> for MapSettings {
    fn from(options: MapOptions) -> Self {
        options.settings()
    }
}

fn parse_source_language(value: &str) -> Result<SourceLanguage, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "rust" => Ok(SourceLanguage::Rust),
        "javascript" | "js" => Ok(SourceLanguage::JavaScript),
        "javascript_jsx" | "jsx" => Ok(SourceLanguage::JavaScriptJsx),
        "typescript" | "ts" => Ok(SourceLanguage::TypeScript),
        "typescript_tsx" | "tsx" => Ok(SourceLanguage::TypeScriptTsx),
        "python" | "py" => Ok(SourceLanguage::Python),
        "ruby" | "rb" => Ok(SourceLanguage::Ruby),
        "java" => Ok(SourceLanguage::Java),
        "go" => Ok(SourceLanguage::Go),
        "lua" => Ok(SourceLanguage::Lua),
        "zig" => Ok(SourceLanguage::Zig),
        "c_sharp" | "csharp" | "c#" => Ok(SourceLanguage::CSharp),
        _ => Err(format!("unsupported source language `{value}`")),
    }
}

impl MapOptions {
    fn settings(&self) -> MapSettings {
        MapSettings {
            excludes: self.excludes.clone(),
            focuses: self.focuses.clone(),
            focus_paths: self.focus_paths.clone(),
            task_seeds: TaskSeeds {
                task: self.task.clone(),
                symbols: self.symbols.clone(),
                paths: self.task_paths.clone(),
                languages: self.languages.clone(),
                projects: self.projects.clone(),
                changes: self
                    .changed_paths
                    .iter()
                    .cloned()
                    .map(TaskChangeSeed::Path)
                    .chain(self.changed_symbols.iter().cloned().map(TaskChangeSeed::Symbol))
                    .collect(),
                search_terms: self.search_terms.clone(),
            },
            map_tokens: self.budget,
            cache_mode: if self.no_cache { CacheMode::Disabled } else { self.cache_mode.into() },
            cache_files: self.cache_files.clone(),
            recursive: self.recursive,
            profile: AnalysisProfile::Compact,
        }
    }

    fn validate(&self) -> Result<(), ApplicationError> {
        if self.budget == 0 {
            return Err(ApplicationError::usage("`--budget` must be greater than zero"));
        }
        if self.no_cache && self.cache_mode != CacheModeOption::Auto {
            return Err(ApplicationError::usage(
                "`--no-cache` cannot be combined with an explicit `--cache` mode",
            ));
        }
        if self.cache_mode == CacheModeOption::Files && self.cache_files.is_empty() && !self.no_cache {
            return Err(ApplicationError::usage(
                "`--cache files` requires at least one `--cache-file` path",
            ));
        }
        if self.cache_files.iter().any(|path| path.trim().is_empty()) {
            return Err(ApplicationError::usage("`--cache-file` paths must not be empty"));
        }
        Ok(())
    }
}

#[derive(Debug, clap::Args)]
#[command(after_help = "Examples:
    dalil history
    dalil history contributors .

Support: https://github.com/stormlightlabs/dalil/issues
")]
struct HistoryCommand {
    #[command(flatten)]
    options: HistoryOptions,

    #[command(subcommand)]
    operation: Option<HistorySubcommand>,

    #[arg(
        value_name = "PATH",
        default_value = ".",
        help = "Repository or subdirectory to analyze (default: current directory)."
    )]
    path: PathBuf,
}

#[derive(Debug, clap::Args)]
#[command(after_help = "Examples:
    dalil history churn
    dalil history bugs --json
    dalil history --html > dalil-history.html

Support: https://github.com/stormlightlabs/dalil/issues
")]
struct HistoryOperationCommand {
    #[command(flatten)]
    options: HistoryOptions,

    #[arg(
        value_name = "PATH",
        default_value = ".",
        help = "Repository or subdirectory to analyze (default: current directory)."
    )]
    path: PathBuf,
}

impl HistorySubcommand {
    fn into_parts(self, inherited: &HistorySettings) -> (HistoryOperation, PathBuf, HistorySettings) {
        match self {
            Self::Churn(command) => (
                HistoryOperation::Churn,
                command.path,
                command.options.settings_with_fallback(inherited),
            ),
            Self::Contributors(command) => (
                HistoryOperation::Contributors,
                command.path,
                command.options.settings_with_fallback(inherited),
            ),
            Self::Bugs(command) => (
                HistoryOperation::Bugs,
                command.path,
                command.options.settings_with_fallback(inherited),
            ),
            Self::Activity(command) => (
                HistoryOperation::Activity,
                command.path,
                command.options.settings_with_fallback(inherited),
            ),
            Self::Firefighting(command) => (
                HistoryOperation::Firefighting,
                command.path,
                command.options.settings_with_fallback(inherited),
            ),
        }
    }
}

#[derive(Clone, Debug, Default, clap::Args)]
struct HistoryOptions {
    /// Number of trailing days for churn, bug, and firefighting signals (default: 365).
    #[arg(long, value_name = "DAYS", value_parser = clap::value_parser!(u32).range(1..))]
    window_days: Option<u32>,

    /// Number of trailing days used for recent contributor concentration (default: 180).
    #[arg(long, value_name = "DAYS", value_parser = clap::value_parser!(u32).range(1..))]
    recent_window_days: Option<u32>,

    /// Replace the default bug-message keywords; repeat for multiple words (default: fix, bug, broken).
    #[arg(long = "bug-keyword", value_name = "WORD", action = ArgAction::Append)]
    bug_keywords: Vec<String>,

    /// Replace the default firefighting keywords; repeat for multiple words (default: revert, hotfix, emergency, rollback).
    #[arg(long = "firefighting-keyword", value_name = "WORD", action = ArgAction::Append)]
    firefighting_keywords: Vec<String>,

    /// Keyword matching policy: word (default) or substring compatibility mode.
    #[arg(long = "keyword-match", value_name = "MODE", value_enum)]
    keyword_match: Option<KeywordMatchModeOption>,

    /// Include contributor email addresses in reports and mailmap provenance.
    #[arg(long, action = ArgAction::SetTrue)]
    include_emails: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum KeywordMatchModeOption {
    Word,
    Substring,
}

impl From<KeywordMatchModeOption> for KeywordMatchMode {
    fn from(mode: KeywordMatchModeOption) -> Self {
        match mode {
            KeywordMatchModeOption::Word => Self::Word,
            KeywordMatchModeOption::Substring => Self::Substring,
        }
    }
}

impl HistoryOptions {
    fn settings(&self) -> HistorySettings {
        self.settings_with_fallback(&HistorySettings::default())
    }

    fn settings_with_fallback(&self, fallback: &HistorySettings) -> HistorySettings {
        HistorySettings {
            window_days: self.window_days.unwrap_or(fallback.window_days),
            recent_window_days: self.recent_window_days.unwrap_or(fallback.recent_window_days),
            bug_keywords: if self.bug_keywords.is_empty() {
                fallback.bug_keywords.clone()
            } else {
                self.bug_keywords.clone()
            },
            firefighting_keywords: if self.firefighting_keywords.is_empty() {
                fallback.firefighting_keywords.clone()
            } else {
                self.firefighting_keywords.clone()
            },
            keyword_match: self.keyword_match.map(Into::into).unwrap_or(fallback.keyword_match),
            include_emails: self.include_emails || fallback.include_emails,
        }
    }
}

#[derive(Debug, clap::Args)]
struct OutputOptions {
    /// Select Markdown, JSON, or a standalone HTML document.
    #[arg(long, global = true, value_enum)]
    format: Option<OutputFormat>,

    /// Shorthand for `--format json`.
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    json: bool,

    /// Shorthand for `--format html`.
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    html: bool,

    /// Open HTML output in the default browser instead of writing it to stdout.
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    open: bool,

    /// Control diagnostic color; report stdout is always uncolored.
    #[arg(long, global = true, value_enum, default_value_t = ColorPolicy::Auto)]
    color: ColorPolicy,

    /// Alias for `--color never`.
    #[arg(long = "no-color", global = true, action = ArgAction::SetTrue)]
    no_color: bool,

    /// Evidence profile: compact (default) or bounded exhaustive evidence.
    #[arg(long, global = true, value_enum, default_value_t = ProfileOption::Compact)]
    profile: ProfileOption,

    /// Fail after rendering when actionable evidence is stale, resource-limited, unsafe, unsupported, or partial.
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    strict: bool,
}

impl OutputOptions {
    fn format(&self) -> Result<OutputFormat, ApplicationError> {
        if self.json && self.html {
            return Err(ApplicationError::usage(
                "`--json` and `--html` cannot be combined; choose one output format",
            ));
        }
        match (self.format, self.json, self.html) {
            (Some(format), true, _) if format != OutputFormat::Json => Err(ApplicationError::usage(
                "`--json` cannot be combined with a different `--format`; choose one output format",
            )),
            (Some(format), _, true) if format != OutputFormat::Html => Err(ApplicationError::usage(
                "`--html` cannot be combined with a different `--format`; choose one output format",
            )),
            (Some(format), _, _) => Ok(format),
            (None, true, false) => Ok(OutputFormat::Json),
            (None, false, true) => Ok(OutputFormat::Html),
            (None, false, false) => Ok(OutputFormat::Markdown),
            (None, true, true) => unreachable!("conflicting shorthand flags returned above"),
        }
    }

    fn color_policy(&self) -> ColorPolicy {
        if self.no_color { ColorPolicy::Never } else { self.color }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ColorEnvironment {
    no_color: bool,
    term_is_dumb: bool,
}

impl ColorEnvironment {
    fn from_process() -> Self {
        Self {
            no_color: std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
            term_is_dumb: matches!(std::env::var("TERM"), Ok(term) if term == "dumb"),
        }
    }
}

#[derive(Debug)]
pub struct CommandRequest {
    pub command: CommandDescriptor,
    pub history: HistorySettings,
    pub map: MapSettings,
    pub context: Option<ContextRequest>,
    pub profile: AnalysisProfile,
    pub output_format: OutputFormat,
    pub color_policy: ColorPolicy,
    pub strict: bool,
}

/// Parse and execute the command line using the process environment and standard streams.
pub fn run() -> i32 {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    let color_environment = ColorEnvironment::from_process();
    let stderr_is_terminal = stderr.is_terminal();

    run_from_with_environment(
        std::env::args_os(),
        &mut stdout,
        &mut stderr,
        stderr_is_terminal,
        color_environment,
    )
    .code()
}

fn run_from_with_environment<I, T, W, E>(
    arguments: I, stdout: &mut W, stderr: &mut E, stderr_is_terminal: bool, color_environment: ColorEnvironment,
) -> ExitCategory
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    W: Write,
    E: Write,
{
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) => {
            return match error.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => match write!(stdout, "{}", error.render()) {
                    Ok(()) => ExitCategory::Success,
                    Err(write_error) => {
                        let _ = writeln!(stderr, "error: could not write command help: {write_error}");
                        ExitCategory::Internal
                    }
                },
                _ => {
                    if write!(stderr, "{}", error.render()).is_ok() {
                        ExitCategory::Usage
                    } else {
                        ExitCategory::Internal
                    }
                }
            };
        }
    };

    let color_policy = cli.color_policy();

    match invoke(cli, stdout, stderr, stderr_is_terminal).context("could not invoke Dalil") {
        Ok(()) => ExitCategory::Success,
        Err(error) => {
            let category = error
                .downcast_ref::<ApplicationError>()
                .map_or(ExitCategory::Internal, |v| v.into());
            write_diagnostic(stderr, &error, color_policy, stderr_is_terminal, color_environment);
            category
        }
    }
}

fn invoke<W: Write, E: Write>(
    cli: Cli, stdout: &mut W, stderr: &mut E, stderr_is_terminal: bool,
) -> anyhow::Result<()> {
    let output_format = cli.output_format()?;
    let open = cli.output.open;
    cli.validate()?;
    if matches!(&cli.command, Some(SubcommandName::Capabilities)) {
        let report = CapabilitiesReport::current();
        let output = report.render(output_format).map_err(ApplicationError::Report)?;
        deliver_output(output_format, open, output, stdout, stderr, "capabilities report")?;
        return Ok(());
    }
    if let Some(SubcommandName::Doctor(command)) = &cli.command {
        let report = DoctorReport::run(command.path.clone());
        let output = report.render(output_format).map_err(ApplicationError::Report)?;
        deliver_output(output_format, open, output, stdout, stderr, "doctor report")?;
        if !report.is_ok() {
            return Err(ApplicationError::DoctorFailed.into());
        }
        return Ok(());
    }
    if let Some(SubcommandName::Cache(cache)) = &cli.command {
        if stderr_is_terminal {
            let _ = writeln!(stderr, "dalil: reading cache metadata…");
        }
        let report = crate::map::cache_control(cache.operation.into())
            .map_err(|error| ApplicationError::Report(crate::report::ReportError::Map(error)))?;
        let output = render_cache_control(&report, output_format).map_err(ApplicationError::Report)?;
        deliver_output(output_format, open, output, stdout, stderr, "cache report")?;
        return Ok(());
    }
    if stderr_is_terminal {
        let _ = writeln!(stderr, "dalil: analyzing repository…");
    }
    let strict = cli.output.strict;
    let report = Report::analyze(cli.into()).map_err(ApplicationError::Report)?;
    let output = report.render(output_format).map_err(ApplicationError::Report)?;
    let strict_issues = report.quality.strict_issues.clone();

    deliver_output(output_format, open, output, stdout, stderr, "report")?;
    if strict && !strict_issues.is_empty() {
        return Err(ApplicationError::Strict { issues: strict_issues }.into());
    }
    Ok(())
}

fn deliver_output<W: Write, E: Write>(
    format: OutputFormat, open: bool, output: String, stdout: &mut W, stderr: &mut E, label: &str,
) -> anyhow::Result<()> {
    if open {
        if format == OutputFormat::Html {
            let path = open_html_report(output.as_bytes())
                .map_err(|(path, error)| ApplicationError::OpenReport { path, error })?;
            let _ = writeln!(stderr, "dalil: opened HTML report at `{}`", path.display());
            return Ok(());
        }
        let _ = writeln!(
            stderr,
            "dalil: warning: `--open` only applies to HTML output; writing {format:?} to stdout"
        );
    }
    write_stdout(stdout, output.as_bytes(), label)
}

fn open_html_report(bytes: &[u8]) -> Result<PathBuf, (PathBuf, io::Error)> {
    let directory = temporary_report_directory();
    create_private_directory(&directory).map_err(|error| (directory.clone(), error))?;
    let path = directory.join("report.html");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|error| (path.clone(), error))?;
    file.write_all(bytes).map_err(|error| (path.clone(), error))?;
    file.flush().map_err(|error| (path.clone(), error))?;

    let status = report_open_command(&path)
        .status()
        .map_err(|error| (path.clone(), error))?;
    if !status.success() {
        return Err((
            path,
            io::Error::other(format!("the platform opener exited with status {status}")),
        ));
    }
    Ok(path)
}

fn temporary_report_directory() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!("dalil-report-{}-{timestamp}", std::process::id()))
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

#[cfg(target_os = "macos")]
fn report_open_command(path: &Path) -> ProcessCommand {
    let mut command = ProcessCommand::new("open");
    command.arg(path);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn report_open_command(path: &Path) -> ProcessCommand {
    let mut command = ProcessCommand::new("xdg-open");
    command.arg(path);
    command
}

#[cfg(windows)]
fn report_open_command(path: &Path) -> ProcessCommand {
    let mut command = ProcessCommand::new("rundll32");
    command.arg("url.dll,FileProtocolHandler").arg(path);
    command
}

fn write_stdout<W: Write>(stdout: &mut W, bytes: &[u8], label: &str) -> anyhow::Result<()> {
    match stdout.write_all(bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
        Err(error) => return Err(error).context(format!("could not write the {label} to stdout")),
    }
    match stdout.flush() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error).context(format!("could not flush {label} stdout")),
    }
}

fn render_cache_control(
    report: &CacheControlReport, format: OutputFormat,
) -> Result<String, crate::report::ReportError> {
    match format {
        OutputFormat::Json => {
            let mut output = serde_json::to_string_pretty(report).map_err(crate::report::ReportError::Json)?;
            output.push('\n');
            Ok(output)
        }
        OutputFormat::Html => crate::report::render_cache_html(report),
        OutputFormat::Markdown => {
            let path = utils::escape_inline_code(report.path.as_deref().unwrap_or("not configured"));
            let mut output = format!("# Dalil cache {}\n\n", report.operation);
            output.push_str(&format!("Path: `{path}`\n"));
            output.push_str(&format!("Exists: {}\n", report.exists));
            output.push_str(&format!(
                "Records: {} ({} bytes) across {} repositories\n",
                report.records, report.bytes, report.repositories
            ));
            output.push_str(&format!(
                "Retention: {} records, {} bytes, {} seconds\n",
                report.max_records_per_repository, report.max_bytes_per_repository, report.max_age_seconds
            ));
            if report.removed_records > 0 {
                output.push_str(&format!(
                    "Removed: {} records ({} bytes)\n",
                    report.removed_records, report.removed_bytes
                ));
            }
            Ok(output)
        }
    }
}

fn write_diagnostic<W: Write, D: std::fmt::Display>(
    stderr: &mut W, error: D, color_policy: ColorPolicy, stderr_is_terminal: bool, color_environment: ColorEnvironment,
) {
    let label = if color_policy.should_color(stderr_is_terminal, color_environment) {
        "error".red().to_string()
    } else {
        "error".to_owned()
    };
    let _ = writeln!(stderr, "{label}: {error:#}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn no_color_alias_overrides_the_color_flag() {
        let options = OutputOptions {
            format: None,
            json: false,
            html: false,
            open: false,
            color: ColorPolicy::Always,
            no_color: true,
            profile: ProfileOption::Compact,
            strict: false,
        };
        assert_eq!(options.color_policy(), ColorPolicy::Never);
    }

    #[test]
    fn auto_color_respects_terminal_and_environment() {
        let neutral = ColorEnvironment { no_color: false, term_is_dumb: false };
        let no_color = ColorEnvironment { no_color: true, term_is_dumb: false };
        let dumb_terminal = ColorEnvironment { no_color: false, term_is_dumb: true };

        assert!(ColorPolicy::Auto.should_color(true, neutral));
        assert!(!ColorPolicy::Auto.should_color(false, neutral));
        assert!(!ColorPolicy::Auto.should_color(true, no_color));
        assert!(!ColorPolicy::Auto.should_color(true, dumb_terminal));
        assert!(ColorPolicy::Always.should_color(false, no_color));
        assert!(!ColorPolicy::Never.should_color(true, neutral));
    }

    #[test]
    fn contradictory_output_flags_are_usage_errors() {
        let options = OutputOptions {
            format: Some(OutputFormat::Markdown),
            json: true,
            html: false,
            open: false,
            color: ColorPolicy::Auto,
            no_color: false,
            profile: ProfileOption::Compact,
            strict: false,
        };

        let error = options.format().expect_err("options should conflict");
        let cat: ExitCategory = error.into();
        assert_eq!(cat, ExitCategory::Usage);
    }

    #[test]
    fn root_and_orient_normalize_to_the_same_orientation_request() {
        let root = CommandRequest::from(
            Cli::try_parse_from(["dalil", "--json", "--task", "inspect parser cache", "."])
                .expect("root orientation flags parse"),
        );
        let orient = CommandRequest::from(
            Cli::try_parse_from(["dalil", "--json", "orient", "--task", "inspect parser cache", "."])
                .expect("orient flags parse"),
        );

        assert_eq!(root.command.name, crate::report::CommandName::Orient);
        assert_eq!(root.command, orient.command);
        assert_eq!(root.history, orient.history);
        assert_eq!(root.output_format, orient.output_format);
        assert_eq!(root.profile, orient.profile);
        assert_eq!(root.map.map_tokens, orient.map.map_tokens);
        assert_eq!(root.map.effective_task_seeds(), orient.map.effective_task_seeds());
    }

    #[test]
    fn task_seed_flags_populate_typed_map_settings() {
        let request = CommandRequest::from(
            Cli::try_parse_from([
                "dalil",
                "map",
                "--task",
                "fix parser cache",
                "--symbol",
                "parse_source",
                "--task-path",
                "src/map",
                "--language",
                "rust",
                "--project",
                "packages/app",
                "--changed-path",
                "src/map/cache.rs",
                "--changed-symbol",
                "CacheStore",
                "--search",
                "invalidation",
            ])
            .expect("task flags parse"),
        );

        assert_eq!(request.map.task_seeds.task.as_deref(), Some("fix parser cache"));
        assert_eq!(request.map.task_seeds.symbols, ["parse_source"]);
        assert_eq!(request.map.task_seeds.paths, ["src/map"]);
        assert_eq!(request.map.task_seeds.languages, [SourceLanguage::Rust]);
        assert_eq!(request.map.task_seeds.projects, ["packages/app"]);
        assert_eq!(
            request.map.task_seeds.changes,
            [
                TaskChangeSeed::Path("src/map/cache.rs".to_owned()),
                TaskChangeSeed::Symbol("CacheStore".to_owned()),
            ]
        );
        assert_eq!(request.map.task_seeds.search_terms, ["invalidation"]);
    }

    #[test]
    fn context_flags_populate_a_typed_request() {
        let request = CommandRequest::from(
            Cli::try_parse_from([
                "dalil",
                "context",
                "--task",
                "  review parser cache  ",
                "--symbol",
                "CacheStore",
                "--task-path",
                "./src/map/",
                "--project",
                "./packages/app/",
                "--changed-path",
                "src/map/cache.rs",
                "--changed-symbol",
                "CacheStore",
                "--base",
                "main~1",
                "--head",
                "HEAD",
                "--dirty-worktree",
                "--teach",
                "--budget",
                "600",
            ])
            .expect("context flags parse"),
        );

        assert_eq!(
            request.command.name,
            CommandDescriptor::context(PathBuf::from(".")).name
        );
        let context = request.context.expect("context request");
        assert_eq!(context.task.as_deref(), Some("  review parser cache  "));
        assert_eq!(context.symbols, ["CacheStore"]);
        assert_eq!(context.paths, ["./src/map/"]);
        assert_eq!(context.projects, ["./packages/app/"]);
        assert_eq!(context.budget, 600);
        assert_eq!(context.revision_context.base.as_deref(), Some("main~1"));
        assert_eq!(context.revision_context.head.as_deref(), Some("HEAD"));
        assert!(context.revision_context.dirty_worktree);
        assert!(context.teaching);
    }

    #[test]
    fn impact_reuses_the_typed_change_context_request() {
        let request = CommandRequest::from(
            Cli::try_parse_from([
                "dalil",
                "impact",
                "--task",
                "review parser changes",
                "--revision-range",
                "main~1..HEAD",
                "--budget",
                "600",
            ])
            .expect("impact flags parse"),
        );

        assert_eq!(request.command.name, CommandDescriptor::impact(PathBuf::from(".")).name);
        let impact = request.context.expect("impact request");
        assert_eq!(impact.task.as_deref(), Some("review parser changes"));
        assert_eq!(impact.revision_context.range.as_deref(), Some("main~1..HEAD"));
        assert_eq!(impact.budget, 600);
        assert!(!impact.teaching);
    }

    #[test]
    fn clap_command_contains_primary_workflows_and_exit_categories() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("orient"));
        assert!(help.contains("map"));
        assert!(help.contains("context"));
        assert!(help.contains("impact"));
        assert!(help.contains("search"));
        assert!(help.contains("explain"));
        assert!(help.contains("history"));
        assert!(help.contains("Exit status:"));
        assert!(help.contains("repository discovery failure"));
    }

    #[test]
    fn orient_help_describes_the_default_workflow() {
        let mut command = Cli::command();
        let help = command
            .find_subcommand_mut("orient")
            .expect("orient command exists")
            .render_long_help()
            .to_string();
        assert!(help.contains("dalil orient"));
        assert!(help.contains("default `dalil` command"));
    }

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn broken_pipe_report_output_is_a_quiet_success() {
        let mut writer = BrokenPipeWriter;
        assert!(write_stdout(&mut writer, b"report", "report").is_ok());
    }
}
