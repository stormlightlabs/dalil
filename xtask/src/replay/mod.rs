//! `cargo replay`: replay commits to measure how well Dalil names the files a change touched.
//!
//! Each eligible commit is a case. Its subject is the task, and the paths it changed are the expected
//! answer. Dalil runs against a temporary checkout of the commit's parent, as does the baseline.

mod baseline;
mod checkout;
mod metrics;

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use clap::Parser;
use gix::objs::tree::EntryKind;
use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use checkout::{CaseDir, TreeFiles, check_interrupted};
use metrics::{CommandSummary, Recall};

/// Commits that change more paths than this are skipped.
pub const MAX_CHANGED_FILES: usize = 50;
/// The most paths kept from each ranked list; recall@10 needs no more.
const KEPT_PATHS: usize = 10;

const CONTEXT: &str = "dalil context";
const SEARCH: &str = "dalil search";
const BASELINE: &str = "baseline";

/// Replay recent commits of a repository against Dalil and a keyword baseline.
#[derive(Debug, Parser)]
#[command(name = "cargo replay")]
struct Args {
    /// Repository to replay.
    #[arg(long)]
    repo: PathBuf,
    /// Number of eligible commits to replay, newest first.
    #[arg(long, default_value_t = 60)]
    commits: usize,
    /// Revision to walk back from.
    #[arg(long, default_value = "HEAD")]
    rev: String,
    /// Dalil binary to run. Without it, `cargo build --release --package dalil` builds one.
    #[arg(long)]
    dalil: Option<PathBuf>,
    /// Directory for `results.json` and `summary.md` (default: `target/replay/<repository name>`).
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub repository: RepositoryInfo,
    pub dalil: DalilInfo,
    pub settings: Settings,
    pub summary: Vec<CommandSummary>,
    pub skipped: Vec<Skipped>,
    pub cases: Vec<Case>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryInfo {
    pub path: String,
    pub revision: String,
    pub head: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DalilInfo {
    pub binary: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub commits_requested: usize,
    pub max_changed_files: usize,
    pub bytes: String,
    pub latency: String,
    pub baseline: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Skipped {
    pub commit: String,
    pub subject: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub commit: String,
    pub parent: String,
    pub subject: String,
    /// Every path the commit added, modified or deleted.
    pub changed: Vec<String>,
    /// Changed paths absent from the parent tree, which no command can name.
    pub added: Vec<String>,
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub command: String,
    /// The exact command line, for Dalil runs.
    pub argv: Option<Vec<String>>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    /// The worktree state Dalil reported for the checkout.
    pub worktree: Option<String>,
    /// Ranked paths, first `KEPT_PATHS` only.
    pub paths: Vec<String>,
    pub bytes: u64,
    pub latency_ms: f64,
    pub recall: Recall,
}

struct Candidate {
    commit: gix::ObjectId,
    parent: gix::ObjectId,
    parent_tree: gix::ObjectId,
    subject: String,
    changed: BTreeSet<String>,
}

pub fn run(arguments: impl Iterator<Item = OsString>) -> io::Result<()> {
    let args = Args::parse_from(std::iter::once(OsString::from("cargo replay")).chain(arguments));
    checkout::install_interrupt_handler();
    let temp_root = std::env::temp_dir();
    for removed in checkout::remove_leftovers(&temp_root)? {
        eprintln!("removed leftover {}", removed.display());
    }

    let dalil = match args.dalil {
        Some(path) => path,
        None => build_dalil()?,
    };
    // Dalil runs from inside each checkout, so a relative path would not resolve.
    let dalil = fs::canonicalize(&dalil)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", dalil.display())))?;
    let version = dalil_version(&dalil)?;
    let source = gix::discover(fs::canonicalize(&args.repo)?).map_err(other)?;
    let head = source
        .rev_parse_single(args.rev.as_str())
        .map_err(other)?
        .object()
        .map_err(other)?
        .peel_to_commit()
        .map_err(other)?
        .id;
    let (candidates, skipped) = select_cases(&source, head, args.commits)?;

    let mut cases = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        check_interrupted()?;
        eprintln!(
            "[{}/{}] {} {}",
            index + 1,
            candidates.len(),
            short(&candidate.commit),
            candidate.subject
        );
        cases.push(replay_case(&source, &dalil, &temp_root, index, candidate)?);
    }

    let name = source
        .workdir()
        .unwrap_or_else(|| source.git_dir())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repository")
        .to_owned();
    let report = Report {
        repository: RepositoryInfo {
            path: args.repo.display().to_string(),
            revision: args.rev.clone(),
            head: head.to_string(),
        },
        dalil: DalilInfo { binary: dalil.display().to_string(), version },
        settings: Settings {
            commits_requested: args.commits,
            max_changed_files: MAX_CHANGED_FILES,
            bytes: "stdout bytes of the --json report; for the baseline, its top 10 paths, one per line".to_owned(),
            latency: "wall time of the process; for the baseline, of reading and ranking the files".to_owned(),
            baseline: "keyword search plus path ranking, see xtask/src/replay/baseline.rs".to_owned(),
        },
        summary: [CONTEXT, SEARCH, BASELINE]
            .iter()
            .map(|command| metrics::summarize(command, &cases))
            .collect(),
        skipped,
        cases,
    };

    let out = args
        .out
        .unwrap_or_else(|| workspace().join("target/replay").join(&name));
    fs::create_dir_all(&out)?;
    let json = serde_json::to_string_pretty(&report).map_err(other)?;
    fs::write(out.join("results.json"), format!("{json}\n"))?;
    let summary = render_summary(&name, &report);
    fs::write(out.join("summary.md"), &summary)?;
    print!("{summary}");
    eprintln!("wrote {}", out.display());
    Ok(())
}

/// Why a commit with this many parents is not a case.
pub fn parent_skip(parents: usize) -> Option<&'static str> {
    match parents {
        0 => Some("root commit"),
        1 => None,
        _ => Some("merge commit"),
    }
}

/// Why a commit changing this many paths is not a case.
pub fn change_skip(changed: usize) -> Option<String> {
    match changed {
        0 => Some("no changed files".to_owned()),
        count if count > MAX_CHANGED_FILES => Some(format!("more than {MAX_CHANGED_FILES} changed files")),
        _ => None,
    }
}

fn select_cases(
    repository: &gix::Repository, head: gix::ObjectId, wanted: usize,
) -> io::Result<(Vec<Candidate>, Vec<Skipped>)> {
    let mut candidates = Vec::new();
    let mut skipped = Vec::new();
    let walk = repository
        .rev_walk([head])
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .all()
        .map_err(other)?;
    for info in walk {
        if candidates.len() >= wanted {
            break;
        }
        let info = info.map_err(other)?;
        let commit = repository.find_commit(info.id).map_err(other)?;
        let subject = commit.message().map_err(other)?.summary().to_string();
        let parents: Vec<gix::ObjectId> = commit.parent_ids().map(|id| id.detach()).collect();
        let skip = |reason: String| Skipped { commit: info.id.to_string(), subject: subject.clone(), reason };
        if let Some(reason) = parent_skip(parents.len()) {
            skipped.push(skip(reason.to_owned()));
            continue;
        }
        let Ok(parent_commit) = repository.find_commit(parents[0]) else {
            skipped.push(skip("parent commit not available".to_owned()));
            continue;
        };
        let parent_tree = parent_commit.tree_id().map_err(other)?.detach();
        let tree = commit.tree_id().map_err(other)?.detach();
        let changed = checkout::changed_paths(repository, parent_tree, tree, MAX_CHANGED_FILES)?;
        if let Some(reason) = change_skip(changed.len()) {
            skipped.push(skip(reason));
            continue;
        }
        candidates.push(Candidate { commit: info.id, parent: parents[0], parent_tree, subject, changed });
    }
    Ok((candidates, skipped))
}

fn replay_case(
    source: &gix::Repository, dalil: &Path, temp_root: &Path, index: usize, candidate: &Candidate,
) -> io::Result<Case> {
    let case_dir = CaseDir::create(temp_root, index)?;
    let checkout = case_dir.path().join("checkout");
    let cache = case_dir.path().join("cache");
    fs::create_dir_all(&checkout)?;
    fs::create_dir_all(&cache)?;
    let files = checkout::flatten_tree(source, candidate.parent_tree)?;
    checkout::materialize(source, candidate.parent, &files, &checkout)?;

    let in_parent: BTreeSet<String> = candidate
        .changed
        .iter()
        .filter(|path| files.contains_key(*path))
        .cloned()
        .collect();
    let added: Vec<String> = candidate.changed.difference(&in_parent).cloned().collect();
    let checkout_arg = checkout.display().to_string();
    let subject = &candidate.subject;

    let context_args = vec![
        "context".to_owned(),
        "--json".to_owned(),
        "--no-cache".to_owned(),
        format!("--task={subject}"),
        checkout_arg.clone(),
    ];
    let mut context = run_dalil(dalil, context_args, &checkout, &cache, context_paths)?;
    context.recall = Recall::of(&context.paths, &candidate.changed, &in_parent);

    let search_args = vec![
        "search".to_owned(),
        "--json".to_owned(),
        "--no-cache".to_owned(),
        "--limit".to_owned(),
        KEPT_PATHS.to_string(),
        "--".to_owned(),
        subject.clone(),
        checkout_arg,
    ];
    let mut search = run_dalil(dalil, search_args, &checkout, &cache, search_paths)?;
    search.recall = Recall::of(&search.paths, &candidate.changed, &in_parent);

    let mut baseline = run_baseline(&checkout, &files, subject);
    baseline.recall = Recall::of(&baseline.paths, &candidate.changed, &in_parent);

    Ok(Case {
        commit: candidate.commit.to_string(),
        parent: candidate.parent.to_string(),
        subject: subject.clone(),
        changed: candidate.changed.iter().cloned().collect(),
        added,
        runs: vec![context, search, baseline],
    })
}

fn run_dalil(
    dalil: &Path, args: Vec<String>, checkout: &Path, cache: &Path, paths: fn(&Value) -> Vec<String>,
) -> io::Result<Run> {
    let command = format!("dalil {}", args[0]);
    let mut argv = vec![dalil.display().to_string()];
    argv.extend(args.iter().cloned());
    let started = Instant::now();
    let output = Command::new(dalil)
        .args(&args)
        .current_dir(checkout)
        .env("XDG_CACHE_HOME", cache)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .output()?;
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    check_interrupted()?;

    let parsed = if output.status.success() {
        serde_json::from_slice::<Value>(&output.stdout).map_err(|error| format!("invalid JSON: {error}"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{}: {}",
            output.status,
            stderr.trim().chars().take(500).collect::<String>()
        ))
    };
    let (paths, worktree, error) = match parsed {
        Ok(value) => (
            metrics::dedupe(paths(&value)).into_iter().take(KEPT_PATHS).collect(),
            value
                .pointer("/provenance/worktree/state")
                .and_then(Value::as_str)
                .map(str::to_owned),
            None,
        ),
        Err(error) => (Vec::new(), None, Some(error)),
    };
    Ok(Run {
        command,
        argv: Some(argv),
        exit_code: output.status.code(),
        error,
        worktree,
        paths,
        bytes: output.stdout.len() as u64,
        latency_ms,
        recall: Recall::default(),
    })
}

fn string_paths<'a>(items: impl Iterator<Item = &'a Value>, pointer: &str) -> Vec<String> {
    items
        .filter_map(|item| item.pointer(pointer).and_then(Value::as_str).map(str::to_owned))
        .collect()
}

/// The recommended files of a `dalil context --json` report, in rank order.
fn context_paths(report: &Value) -> Vec<String> {
    let files = report.pointer("/context/files").and_then(Value::as_array);
    string_paths(files.into_iter().flatten(), "/recommendation/path")
}

/// The result paths of a `dalil search --json` report, in rank order.
fn search_paths(report: &Value) -> Vec<String> {
    let matches = report.pointer("/search/matches").and_then(Value::as_array);
    string_paths(matches.into_iter().flatten(), "/path")
}

fn run_baseline(checkout: &Path, files: &TreeFiles, subject: &str) -> Run {
    let started = Instant::now();
    let words = baseline::tokens(subject);
    let scored = files
        .iter()
        .filter(|(_, (kind, _))| matches!(kind, EntryKind::Blob | EntryKind::BlobExecutable))
        .map(|(path, _)| {
            let content = fs::read(checkout.join(path)).ok();
            (path.clone(), baseline::score(path, content.as_deref(), &words))
        })
        .collect();
    let paths: Vec<String> = baseline::rank(scored).into_iter().take(KEPT_PATHS).collect();
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    let bytes = paths.iter().map(|path| path.len() as u64 + 1).sum();
    Run {
        command: BASELINE.to_owned(),
        argv: None,
        exit_code: None,
        error: None,
        worktree: None,
        paths,
        bytes,
        latency_ms,
        recall: Recall::default(),
    }
}

fn render_summary(name: &str, report: &Report) -> String {
    let mut reasons = std::collections::BTreeMap::<&str, usize>::new();
    for skipped in &report.skipped {
        *reasons.entry(skipped.reason.as_str()).or_default() += 1;
    }
    let reasons = reasons
        .iter()
        .map(|(reason, count)| format!("{reason}: {count}"))
        .collect::<Vec<_>>();
    let percent = |value: Option<f64>| value.map_or("n/a".to_owned(), |value| format!("{:.1}%", value * 100.0));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Replay of {name} at {}\n",
        &report.repository.head[..12.min(report.repository.head.len())]
    );
    let _ = writeln!(
        out,
        "{} cases replayed, {} commits skipped{}.\n",
        report.cases.len(),
        report.skipped.len(),
        if reasons.is_empty() { String::new() } else { format!(" ({})", reasons.join(", ")) }
    );
    let _ = writeln!(
        out,
        "| Command | Cases | Failed | R@5 | R@10 | R@5 in parent | R@10 in parent | Median bytes | Median ms |"
    );
    let _ = writeln!(out, "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    for summary in &report.summary {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            summary.command,
            summary.cases,
            summary.failed,
            percent(summary.mean_recall_all_at_5),
            percent(summary.mean_recall_all_at_10),
            percent(summary.mean_recall_parent_at_5),
            percent(summary.mean_recall_parent_at_10),
            summary
                .median_bytes
                .map_or("n/a".to_owned(), |bytes| format!("{bytes:.0}")),
            summary
                .median_latency_ms
                .map_or("n/a".to_owned(), |ms| format!("{ms:.0}")),
        );
    }
    let parent_cases = report.summary.first().map_or(0, |summary| summary.parent_cases);
    let _ = writeln!(
        out,
        "\nR@k counts every changed path. \"In parent\" counts only paths present in the parent tree, over the {parent_cases} cases that have one."
    );
    out
}

fn build_dalil() -> io::Result<PathBuf> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output = Command::new(cargo)
        .args([
            "build",
            "--release",
            "--package",
            "dalil",
            "--bin",
            "dalil",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(workspace())
        .stderr(Stdio::inherit())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!("building dalil failed: {}", output.status)));
    }
    artifact_executable(&output.stdout, "dalil").ok_or_else(|| io::Error::other("cargo reported no dalil executable"))
}

/// The executable cargo's JSON messages report for the binary target `name`.
fn artifact_executable(messages: &[u8], name: &str) -> Option<PathBuf> {
    String::from_utf8_lossy(messages)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact" && message["target"]["name"] == name)
        .find_map(|message| message["executable"].as_str().map(PathBuf::from))
}

fn dalil_version(dalil: &Path) -> io::Result<String> {
    let output = Command::new(dalil).arg("--version").output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a direct child of the workspace")
}

fn short(id: &gix::ObjectId) -> String {
    id.to_hex_with_len(10).to_string()
}

pub(crate) fn other(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_roots_and_large_or_empty_commits_are_skipped() {
        assert_eq!(parent_skip(0), Some("root commit"));
        assert_eq!(parent_skip(1), None);
        assert_eq!(parent_skip(2), Some("merge commit"));
        assert_eq!(change_skip(0).as_deref(), Some("no changed files"));
        assert_eq!(change_skip(1), None);
        assert_eq!(change_skip(MAX_CHANGED_FILES), None);
        assert_eq!(
            change_skip(MAX_CHANGED_FILES + 1).as_deref(),
            Some("more than 50 changed files")
        );
    }

    #[test]
    fn report_paths_come_out_in_rank_order() {
        let context = serde_json::json!({"context": {"files": [
            {"recommendation": {"path": "b.rs"}},
            {"recommendation": {"path": "a.rs"}}
        ]}});
        assert_eq!(context_paths(&context), ["b.rs", "a.rs"]);
        let search = serde_json::json!({"search": {"matches": [{"path": "x.rs"}, {"path": "x.rs"}, {"path": "y.rs"}]}});
        assert_eq!(metrics::dedupe(search_paths(&search)), ["x.rs", "y.rs"]);
        assert!(context_paths(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn artifact_executable_reads_cargo_messages() {
        let messages = concat!(
            "{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"dalil_core\"},\"executable\":null}\n",
            "not json\n",
            "{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"dalil\"},\"executable\":\"/t/dalil\"}\n",
        );
        assert_eq!(
            artifact_executable(messages.as_bytes(), "dalil"),
            Some(PathBuf::from("/t/dalil"))
        );
        assert_eq!(artifact_executable(b"", "dalil"), None);
    }
}
