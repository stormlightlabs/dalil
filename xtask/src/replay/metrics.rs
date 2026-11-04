//! Recall, medians and the per-command summary.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{Case, Run};

/// Share of `expected` found in the first `k` of `ranked`, or `None` when nothing is expected.
pub fn recall_at(ranked: &[String], expected: &BTreeSet<String>, k: usize) -> Option<f64> {
    if expected.is_empty() {
        return None;
    }
    let hits = ranked.iter().take(k).filter(|path| expected.contains(*path)).count();
    Some(hits as f64 / expected.len() as f64)
}

/// Drop repeated paths, keeping the first position of each.
pub fn dedupe(paths: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    paths.into_iter().filter(|path| seen.insert(path.clone())).collect()
}

pub fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len() % 2 == 0 { (values[middle - 1] + values[middle]) / 2.0 } else { values[middle] })
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() { None } else { Some(values.iter().sum::<f64>() / values.len() as f64) }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Recall {
    /// Over every path the commit changed.
    pub all_at_5: Option<f64>,
    pub all_at_10: Option<f64>,
    /// Over the changed paths that exist in the parent tree.
    pub parent_at_5: Option<f64>,
    pub parent_at_10: Option<f64>,
}

impl Recall {
    pub fn of(ranked: &[String], all: &BTreeSet<String>, in_parent: &BTreeSet<String>) -> Self {
        Self {
            all_at_5: recall_at(ranked, all, 5),
            all_at_10: recall_at(ranked, all, 10),
            parent_at_5: recall_at(ranked, in_parent, 5),
            parent_at_10: recall_at(ranked, in_parent, 10),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandSummary {
    pub command: String,
    pub cases: usize,
    pub failed: usize,
    /// Cases with at least one changed path in the parent tree; the `parent` means cover only these.
    pub parent_cases: usize,
    pub mean_recall_all_at_5: Option<f64>,
    pub mean_recall_all_at_10: Option<f64>,
    pub mean_recall_parent_at_5: Option<f64>,
    pub mean_recall_parent_at_10: Option<f64>,
    pub median_bytes: Option<f64>,
    pub median_latency_ms: Option<f64>,
}

/// Summarize one command across cases. Failed runs count as zero recall; bytes and latency cover
/// successful runs only.
pub fn summarize(command: &str, cases: &[Case]) -> CommandSummary {
    let runs: Vec<&Run> = cases
        .iter()
        .filter_map(|case| case.runs.iter().find(|run| run.command == command))
        .collect();
    let pick = |field: fn(&Recall) -> Option<f64>| runs.iter().filter_map(|run| field(&run.recall)).collect::<Vec<_>>();
    let ok: Vec<&&Run> = runs.iter().filter(|run| run.error.is_none()).collect();
    CommandSummary {
        command: command.to_owned(),
        cases: runs.len(),
        failed: runs.len() - ok.len(),
        parent_cases: pick(|recall| recall.parent_at_5).len(),
        mean_recall_all_at_5: mean(&pick(|recall| recall.all_at_5)),
        mean_recall_all_at_10: mean(&pick(|recall| recall.all_at_10)),
        mean_recall_parent_at_5: mean(&pick(|recall| recall.parent_at_5)),
        mean_recall_parent_at_10: mean(&pick(|recall| recall.parent_at_10)),
        median_bytes: median(ok.iter().map(|run| run.bytes as f64).collect()),
        median_latency_ms: median(ok.iter().map(|run| run.latency_ms).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    fn list(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    #[test]
    fn recall_counts_expected_paths_in_the_first_k() {
        let ranked = list(&["a", "x", "b", "y", "z", "c"]);
        let expected = set(&["a", "b", "c", "d"]);
        assert_eq!(recall_at(&ranked, &expected, 5), Some(0.5));
        assert_eq!(recall_at(&ranked, &expected, 10), Some(0.75));
        assert_eq!(recall_at(&ranked, &expected, 1), Some(0.25));
        assert_eq!(recall_at(&[], &expected, 5), Some(0.0));
        assert_eq!(recall_at(&ranked, &BTreeSet::new(), 5), None);
    }

    #[test]
    fn recall_variants_separate_added_files() {
        let ranked = list(&["src/old.rs"]);
        let recall = Recall::of(&ranked, &set(&["src/old.rs", "src/new.rs"]), &set(&["src/old.rs"]));
        assert_eq!(recall.all_at_5, Some(0.5));
        assert_eq!(recall.parent_at_5, Some(1.0));
        let only_added = Recall::of(&ranked, &set(&["src/new.rs"]), &BTreeSet::new());
        assert_eq!(only_added.all_at_10, Some(0.0));
        assert_eq!(only_added.parent_at_10, None);
    }

    #[test]
    fn dedupe_keeps_first_position() {
        assert_eq!(dedupe(list(&["a", "b", "a", "c", "b"])), ["a", "b", "c"]);
    }

    #[test]
    fn median_handles_odd_even_and_empty() {
        assert_eq!(median(vec![3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(vec![4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(Vec::new()), None);
    }

    #[test]
    fn summary_counts_failures_as_zero_recall() {
        let run = |command: &str, recall: Recall, error: Option<&str>, bytes: u64| Run {
            command: command.to_owned(),
            argv: None,
            exit_code: Some(0),
            error: error.map(str::to_owned),
            worktree: None,
            paths: Vec::new(),
            bytes,
            latency_ms: 10.0,
            recall,
        };
        let full =
            Recall { all_at_5: Some(1.0), all_at_10: Some(1.0), parent_at_5: Some(1.0), parent_at_10: Some(1.0) };
        let zero = Recall { all_at_5: Some(0.0), all_at_10: Some(0.0), parent_at_5: None, parent_at_10: None };
        let case = |runs| Case {
            commit: String::new(),
            parent: String::new(),
            subject: String::new(),
            changed: Vec::new(),
            added: Vec::new(),
            runs,
        };
        let cases = vec![
            case(vec![run("search", full, None, 100)]),
            case(vec![run("search", zero, Some("exit 5"), 7)]),
        ];
        let summary = summarize("search", &cases);
        assert_eq!(summary.cases, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.parent_cases, 1);
        assert_eq!(summary.mean_recall_all_at_5, Some(0.5));
        assert_eq!(summary.mean_recall_parent_at_10, Some(1.0));
        assert_eq!(summary.median_bytes, Some(100.0));
    }
}
