use std::collections::{BTreeMap, BTreeSet, VecDeque};

use petgraph::Direction;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;

use super::relationships::RelationshipGraph;
use super::*;
use crate::utils::token_count;

const MAX_SYMBOLS_PER_TARGET: usize = 3;
const MAX_TESTS: usize = 16;
const MAX_OWNERSHIP_SIGNALS: usize = 4;
const MAX_HISTORY_EVIDENCE: usize = 8;
const MAX_UNCERTAINTIES: usize = 6;
const MAX_IMPACT_DEPTH: usize = 16;
const MAX_IMPACT_WORK: usize = 20_000;
const MAX_IMPACT_PROJECTS: usize = 32;

#[derive(Default)]
struct TargetCandidate {
    symbols: Vec<ContextSymbol>,
    evidence: BTreeSet<ImpactEvidenceKind>,
    score: u64,
    confidence: ConfidenceTier,
    reasons: BTreeSet<String>,
    limitations: Vec<String>,
    reachability: ImpactReachability,
    depth: usize,
    project_root: Option<String>,
    relationship_path: Vec<RepositoryRelationship>,
}

struct TargetInput {
    path: String,
    source: ReadingSourceEvidence,
    rank: Option<FileRank>,
    evidence: ImpactEvidenceKind,
    priority: u64,
    confidence: ConfidenceTier,
    reason: String,
    changed_symbols: Option<BTreeSet<String>>,
    impact_symbol: Option<SourceSymbol>,
    reachability: ImpactReachability,
    depth: usize,
    relationship_path: Vec<RepositoryRelationship>,
    project_root: Option<String>,
}

#[derive(Clone)]
struct ImpactSeedInput {
    kind: ImpactSeedKind,
    path: String,
    symbol: Option<String>,
    reason: String,
    nodes: Vec<NodeIndex>,
}

#[derive(Clone)]
struct ImpactWalkTarget {
    index: NodeIndex,
    depth: usize,
    reachability: ImpactReachability,
    relationship_path: Vec<RepositoryRelationship>,
}

struct ImpactWalk {
    seeds: Vec<ImpactSeed>,
    seed_inputs: Vec<ImpactSeedInput>,
    targets: Vec<ImpactWalkTarget>,
    related_targets: Vec<ImpactWalkTarget>,
    related_relationships: Vec<ImpactRelationship>,
    bounds: ImpactTraversalBounds,
    unresolved: Vec<String>,
}

/// Compile evidence around resolved changes into a review result. The
/// operation reports observed relationships and their limits, rather than
/// claiming semantic reachability or breakage.
pub fn compile(
    request: ContextRequest, map: &MapReport, history: &HistoryReport, change_resolution: ChangeResolution,
) -> ImpactReport {
    let request = normalized_request(request, map, &change_resolution);
    let budget = request.budget;
    let changed_paths = changed_paths(&change_resolution);
    let changed_symbols = changed_symbols(&change_resolution);
    let sources = source_evidence(map);
    let edges = graph_evidence(map);
    let ranks = rank_evidence(map);
    let source_by_path = sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let rank_by_path = ranks
        .iter()
        .map(|rank| (rank.path.as_str(), rank))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = BTreeMap::<String, TargetCandidate>::new();
    let mut relationships = Vec::new();

    for path in &changed_paths {
        let Some(source) = source_by_path.get(path.as_str()) else {
            continue;
        };
        add_target(
            &mut candidates,
            TargetInput {
                path: path.clone(),
                source: (*source).clone(),
                rank: rank_by_path.get(path.as_str()).copied().cloned(),
                evidence: ImpactEvidenceKind::Structural,
                priority: 4_000_000_000,
                confidence: ConfidenceTier::High,
                reason: "the resolved change includes this current source path".to_owned(),
                changed_symbols: changed_symbols.get(path).cloned(),
                impact_symbol: None,
                reachability: ImpactReachability::Direct,
                depth: 0,
                relationship_path: Vec::new(),
                project_root: project_root_for_path(map, path),
            },
        );
    }

    // Preserve the previous lexical relationship collection. Graph traversal
    // owns target ranking; this compatibility evidence keeps ambiguous edges
    // visible even when the graph cannot construct both endpoint nodes.
    for edge in &edges {
        let related = changed_paths.contains(&edge.source)
            || changed_paths.contains(&edge.target)
            || changed_symbols
                .get(&edge.target)
                .is_some_and(|symbols| symbols.contains(&edge.symbol));
        if related {
            let downstream = changed_paths.contains(&edge.target)
                || changed_symbols
                    .get(&edge.target)
                    .is_some_and(|symbols| symbols.contains(&edge.symbol));
            relationships.push(lexical_relationship(
                edge,
                if downstream { ImpactReachability::Direct } else { ImpactReachability::Inferred },
            ));
        }
    }

    let graph = RelationshipGraph::build(map);
    let walk = walk_impact(&graph, &request, &change_resolution);
    let impact_seeds = walk.seeds.clone();
    for seed in &walk.seed_inputs {
        for index in &seed.nodes {
            let node = graph.node(*index);
            let Some(source) = source_by_path.get(node.path.as_str()) else {
                continue;
            };
            let rank = rank_by_path.get(node.path.as_str()).copied().cloned();
            add_target(
                &mut candidates,
                TargetInput {
                    path: node.path.clone(),
                    source: (*source).clone(),
                    rank,
                    evidence: ImpactEvidenceKind::Structural,
                    priority: 4_000_000_000,
                    confidence: if node.partial { ConfidenceTier::Medium } else { ConfidenceTier::High },
                    reason: seed.reason.clone(),
                    changed_symbols: changed_symbols.get(&node.path).cloned(),
                    impact_symbol: node.symbol.clone(),
                    reachability: ImpactReachability::Direct,
                    depth: 0,
                    relationship_path: Vec::new(),
                    project_root: node.project_root.clone(),
                },
            );
        }
    }

    let mut graph_relationships = walk
        .related_relationships
        .iter()
        .map(|relationship| {
            (
                relationship.relationship_id.clone().unwrap_or_default(),
                relationship.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for reached in walk.targets.iter().chain(&walk.related_targets) {
        let node = graph.node(reached.index);
        let Some(source) = source_by_path.get(node.path.as_str()) else {
            continue;
        };
        let rank = rank_by_path.get(node.path.as_str()).copied().cloned();
        let confidence = reached
            .relationship_path
            .iter()
            .map(|relationship| relationship.confidence)
            .min()
            .unwrap_or(if node.partial { ConfidenceTier::Medium } else { ConfidenceTier::High });
        add_target(
            &mut candidates,
            TargetInput {
                path: node.path.clone(),
                source: (*source).clone(),
                rank,
                evidence: ImpactEvidenceKind::Lexical,
                priority: impact_priority(reached.depth, reached.reachability),
                confidence,
                reason: format!(
                    "a {} downstream graph path reaches this path at depth {}",
                    reached.reachability.label(),
                    reached.depth
                ),
                changed_symbols: changed_symbols.get(&node.path).cloned(),
                impact_symbol: node.symbol.clone(),
                reachability: reached.reachability,
                depth: reached.depth,
                relationship_path: reached.relationship_path.clone(),
                project_root: node.project_root.clone(),
            },
        );
        let target_path = reached.depth;
        for relationship in &reached.relationship_path {
            let entry = graph_relationships
                .entry(relationship.id.clone())
                .or_insert_with(|| graph_impact_relationship(relationship, reached.reachability, target_path));
            if impact_reachability_rank(reached.reachability) > impact_reachability_rank(entry.reachability)
                || (reached.reachability == entry.reachability && target_path < entry.depth)
            {
                *entry = graph_impact_relationship(relationship, reached.reachability, target_path);
            }
        }
    }
    relationships.extend(graph_relationships.into_values());

    for root in &map.project_roots {
        for metadata in &root.manifest_metadata {
            for target in metadata.runtime_entry_points.iter().chain(&metadata.library_exports) {
                let Some(path) = &target.resolved_path else {
                    continue;
                };
                if !changed_paths.contains(path) {
                    continue;
                }
                relationships.push(ImpactRelationship {
                    source: metadata.path.clone(),
                    target: path.clone(),
                    evidence: ImpactEvidenceKind::Manifest,
                    confidence: ConfidenceTier::High,
                    reason: format!("the manifest declares `{}` as a target", target.declared),
                    symbol: target.name.clone(),
                    ambiguous: false,
                    reachability: ImpactReachability::Direct,
                    depth: 0,
                    relationship_id: None,
                });
                if let Some(source) = source_by_path.get(path.as_str()) {
                    add_target(
                        &mut candidates,
                        TargetInput {
                            path: path.clone(),
                            source: (*source).clone(),
                            rank: rank_by_path.get(path.as_str()).copied().cloned(),
                            evidence: ImpactEvidenceKind::Manifest,
                            priority: 3_500_000_000,
                            confidence: ConfidenceTier::High,
                            reason: "a project manifest declares this changed path as a runtime or library target"
                                .to_owned(),
                            changed_symbols: changed_symbols.get(path).cloned(),
                            impact_symbol: None,
                            reachability: ImpactReachability::Direct,
                            depth: 0,
                            relationship_path: Vec::new(),
                            project_root: project_root_for_path(map, path),
                        },
                    );
                }
            }
        }
    }

    let mut tests = likely_tests(
        map,
        &changed_paths,
        &source_by_path,
        &rank_by_path,
        &mut candidates,
        &mut relationships,
    );
    for reached in &walk.targets {
        let node = graph.node(reached.index);
        if !is_test_path(&node.path) {
            continue;
        }
        let reason = format!(
            "a {} downstream graph path reaches this test at depth {}",
            reached.reachability.label(),
            reached.depth
        );
        add_impact_test(
            &mut tests,
            ContextTest {
                path: node.path,
                reason,
                confidence: reached
                    .relationship_path
                    .iter()
                    .map(|relationship| relationship.confidence)
                    .min()
                    .unwrap_or(ConfidenceTier::Low),
                score: Some(impact_priority(reached.depth, reached.reachability)),
                depth: Some(reached.depth),
                reachability: Some(reached.reachability),
            },
        );
    }
    let ownership = ownership_signals(map, &changed_paths);
    let history_complete = history.provenance.completeness.status == HistoryCompletenessStatus::Complete;
    let history = history_evidence(history, &changed_paths, &mut relationships);
    relationships.sort_by(|left, right| {
        left.evidence
            .cmp(&right.evidence)
            .then_with(|| {
                impact_reachability_rank(right.reachability).cmp(&impact_reachability_rank(left.reachability))
            })
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.depth.cmp(&right.depth))
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.symbol.cmp(&right.symbol))
    });
    relationships.dedup();
    let target_total = candidates.len();
    let relationship_total = relationships.len();
    let test_total = tests.len();
    let ownership_total = ownership.len();
    let history_total = history.len();

    let uncertainty = uncertainty(
        map,
        history_complete,
        &change_resolution,
        &walk.bounds,
        &walk.unresolved,
    );
    let mut report = ImpactReport {
        request,
        change_resolution,
        seeds: impact_seeds,
        traversal: walk.bounds,
        budget: ContextBudget { token_budget: budget, estimated_tokens: 0, truncated: false },
        ..ImpactReport::default()
    };

    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_by(|(left_path, left), (right_path, right)| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                impact_reachability_rank(right.reachability).cmp(&impact_reachability_rank(left.reachability))
            })
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left_path.cmp(right_path))
    });
    let mut all_targets = Vec::new();
    for (path, candidate) in candidates {
        all_targets.push(ImpactTarget {
            path,
            symbols: candidate.symbols,
            evidence: candidate.evidence.into_iter().collect(),
            confidence: candidate.confidence,
            score: candidate.score,
            reason: candidate.reasons.into_iter().take(2).collect::<Vec<_>>().join("; "),
            reachability: candidate.reachability,
            depth: candidate.depth,
            project_root: candidate.project_root,
            relationship_path: candidate.relationship_path,
            limitations: candidate.limitations,
        });
    }
    for target in all_targets.iter().cloned() {
        add_if_fits(&mut report, budget, |report| report.targets.push(target));
    }
    report.projects = project_summaries(&all_targets, &tests);
    let project_total = report.projects.len();
    let projects = std::mem::take(&mut report.projects);
    for project in projects {
        add_if_fits(&mut report, budget, |report| report.projects.push(project));
    }
    for relationship in relationships {
        add_if_fits(&mut report, budget, |report| report.relationships.push(relationship));
    }
    for test in tests {
        add_if_fits(&mut report, budget, |report| report.likely_tests.push(test));
    }
    for signal in ownership {
        add_if_fits(&mut report, budget, |report| report.ownership.push(signal));
    }
    for evidence in history {
        add_if_fits(&mut report, budget, |report| report.history.push(evidence));
    }
    for item in uncertainty {
        add_if_fits(&mut report, budget, |report| report.uncertainty.push(item));
    }

    report.budget.estimated_tokens = estimate_tokens(&report);
    report.budget.truncated = report.budget.estimated_tokens > budget
        || report.targets.len() < target_total
        || report.projects.len() < project_total
        || report.relationships.len() < relationship_total
        || report.likely_tests.len() < test_total
        || report.ownership.len() < ownership_total
        || report.history.len() < history_total;
    report
}

fn normalized_request(
    request: ContextRequest, map: &MapReport, change_resolution: &ChangeResolution,
) -> ContextRequest {
    ContextRequest {
        repository: map.repository_root.clone(),
        task: map.task_seeds.task.clone(),
        symbols: map.task_seeds.symbols.clone(),
        paths: map.task_seeds.paths.clone(),
        projects: map.task_seeds.projects.clone(),
        changes: map.task_seeds.changes.clone(),
        revision_context: request.revision_context,
        change_resolution: change_resolution.clone(),
        budget: request.budget,
        profile: request.profile,
        teaching: false,
    }
}

fn changed_paths(resolution: &ChangeResolution) -> BTreeSet<String> {
    resolution
        .changes
        .iter()
        .flat_map(|change| std::iter::once(change.path.clone()).chain(change.previous_path.clone()))
        .collect()
}

fn changed_symbols(resolution: &ChangeResolution) -> BTreeMap<String, BTreeSet<String>> {
    resolution
        .changes
        .iter()
        .map(|change| {
            (
                change.path.clone(),
                change
                    .symbols
                    .iter()
                    .map(|symbol| symbol.name.clone())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect()
}

fn source_evidence(map: &MapReport) -> Vec<ReadingSourceEvidence> {
    if map.reading_evidence.sources.is_empty() {
        map.files
            .iter()
            .map(|file| ReadingSourceEvidence {
                path: file.path.clone(),
                language: file.language,
                worktree_state: file.worktree_state,
                status: file.status,
                symbols: file.symbols.clone(),
                limitations: file.limitations.clone(),
            })
            .collect()
    } else {
        map.reading_evidence.sources.clone()
    }
}

fn graph_evidence(map: &MapReport) -> Vec<LexicalEdge> {
    if map.reading_evidence.graph.is_empty() {
        map.edges.clone()
    } else {
        map.reading_evidence
            .graph
            .iter()
            .map(|relationship| relationship.relationship.clone())
            .collect()
    }
}

fn rank_evidence(map: &MapReport) -> &[FileRank] {
    if map.reading_evidence.ranking.is_empty() { &map.ranking } else { &map.reading_evidence.ranking }
}

fn add_target(candidates: &mut BTreeMap<String, TargetCandidate>, input: TargetInput) {
    let candidate = candidates.entry(input.path.clone()).or_default();
    let score = input
        .priority
        .saturating_add(input.rank.as_ref().map_or(0, |rank| rank.score));
    let replace_path = score > candidate.score
        || (score == candidate.score
            && impact_reachability_rank(input.reachability) > impact_reachability_rank(candidate.reachability))
        || (score == candidate.score && input.depth < candidate.depth)
        || (score == candidate.score && candidate.relationship_path.is_empty() && !input.relationship_path.is_empty());
    candidate.evidence.insert(input.evidence);
    candidate.score = candidate.score.max(score);
    candidate.confidence = candidate.confidence.max(input.confidence);
    candidate.reasons.insert(input.reason);
    if replace_path {
        candidate.reachability = input.reachability;
        candidate.depth = input.depth;
        candidate.relationship_path = input.relationship_path;
    }
    if candidate.project_root.is_none() {
        candidate.project_root = input.project_root;
    }
    if candidate.limitations.is_empty() {
        candidate.limitations = input.source.limitations.clone();
    }
    if let Some(symbol) = input.impact_symbol {
        candidate.symbols.push(ContextSymbol {
            path: input.path.clone(),
            symbol,
            score: input.rank.as_ref().map_or(0, |rank| rank.score),
        });
    }
    if candidate.symbols.is_empty() {
        let mut symbols = input.source.symbols.iter().collect::<Vec<_>>();
        symbols.sort_by(|left, right| {
            let left_changed = input
                .changed_symbols
                .as_ref()
                .is_some_and(|names| names.contains(&left.name));
            let right_changed = input
                .changed_symbols
                .as_ref()
                .is_some_and(|names| names.contains(&right.name));
            right_changed
                .cmp(&left_changed)
                .then_with(|| (right.role == SymbolRole::Definition).cmp(&(left.role == SymbolRole::Definition)))
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.location.start.line.cmp(&right.location.start.line))
        });
        candidate.symbols = symbols
            .into_iter()
            .take(MAX_SYMBOLS_PER_TARGET)
            .map(|symbol| ContextSymbol {
                path: input.path.clone(),
                symbol: symbol.clone(),
                score: input.rank.as_ref().map_or(0, |rank| rank.score),
            })
            .collect();
    }
    candidate.symbols.sort_by(|left, right| {
        let left_impact = left.symbol.role == SymbolRole::Reference;
        let right_impact = right.symbol.role == SymbolRole::Reference;
        right_impact
            .cmp(&left_impact)
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| left.symbol.name.cmp(&right.symbol.name))
            .then_with(|| left.symbol.location.start.line.cmp(&right.symbol.location.start.line))
    });
    candidate
        .symbols
        .dedup_by(|left, right| left.symbol.name == right.symbol.name && left.symbol.location == right.symbol.location);
    candidate.symbols.truncate(MAX_SYMBOLS_PER_TARGET);
}

fn lexical_relationship(edge: &LexicalEdge, reachability: ImpactReachability) -> ImpactRelationship {
    ImpactRelationship {
        source: edge.source.clone(),
        target: edge.target.clone(),
        evidence: ImpactEvidenceKind::Lexical,
        confidence: edge.confidence,
        reason: format!(
            "a {} lexical reference resolved by {}",
            edge.symbol,
            edge.resolution_reason.label()
        ),
        symbol: Some(edge.symbol.clone()),
        ambiguous: edge.ambiguous,
        reachability,
        depth: 1,
        relationship_id: None,
    }
}

fn graph_impact_relationship(
    relationship: &RepositoryRelationship, reachability: ImpactReachability, depth: usize,
) -> ImpactRelationship {
    ImpactRelationship {
        source: relationship.source_path.clone(),
        target: relationship.target_path.clone(),
        evidence: ImpactEvidenceKind::Lexical,
        confidence: relationship.confidence,
        reason: format!(
            "a retained {} graph edge is on a {} downstream path at depth {}",
            relationship.kind.label(),
            reachability.label(),
            depth
        ),
        symbol: relationship.symbol.clone(),
        ambiguous: relationship.ambiguous,
        reachability,
        depth,
        relationship_id: Some(relationship.id.clone()),
    }
}

fn impact_priority(depth: usize, reachability: ImpactReachability) -> u64 {
    let base: u64 = match reachability {
        ImpactReachability::Direct => 3_000_000_000,
        ImpactReachability::Transitive => 2_000_000_000,
        ImpactReachability::Inferred => 1_000_000_000,
    };
    base.saturating_sub((depth as u64).saturating_sub(1).saturating_mul(75_000_000))
}

fn impact_reachability_rank(reachability: ImpactReachability) -> u8 {
    match reachability {
        ImpactReachability::Direct => 3,
        ImpactReachability::Transitive => 2,
        ImpactReachability::Inferred => 1,
    }
}

fn walk_impact(graph: &RelationshipGraph, request: &ContextRequest, resolution: &ChangeResolution) -> ImpactWalk {
    let seed_inputs = impact_seed_inputs(graph, request, resolution);
    let mut seeds = Vec::new();
    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::<NodeIndex>::new();
    let mut seed_nodes = BTreeSet::new();
    let mut unresolved = Vec::new();

    for input in &seed_inputs {
        let mut nodes = input.nodes.clone();
        nodes.sort_by_key(|index| graph.node(*index).id);
        nodes.dedup();
        if nodes.is_empty() {
            unresolved.push(match &input.symbol {
                Some(symbol) => format!(
                    "`{symbol}` in `{}` did not resolve to a retained graph symbol",
                    input.path
                ),
                None => format!("`{}` did not resolve to a retained graph file", input.path),
            });
        }
        let node_ids = nodes.iter().map(|index| graph.node(*index).id).collect::<Vec<_>>();
        seeds.push(ImpactSeed {
            kind: input.kind,
            path: input.path.clone(),
            symbol: input.symbol.clone(),
            node_ids,
            reason: input.reason.clone(),
        });
        for index in nodes {
            seed_nodes.insert(index);
            if visited.insert(index) {
                queue.push_back((index, 0usize, Vec::new(), ImpactReachability::Direct));
            }
        }
    }

    // Capture all one-edge evidence around each seed through the shared graph.
    // Incoming edges are downstream evidence; outgoing edges remain visible as
    // inferred related evidence for compatibility with the original report.
    let mut related_targets = BTreeMap::<NodeIndex, ImpactWalkTarget>::new();
    let mut related_relationships = BTreeMap::<String, ImpactRelationship>::new();
    let mut work_used = 0;
    let mut inspected_edges = 0;
    let mut work_limited = false;
    'related: for seed in &seed_nodes {
        for direction in [Direction::Incoming, Direction::Outgoing] {
            for edge in graph.graph.edges_directed(*seed, direction) {
                if work_used >= MAX_IMPACT_WORK {
                    work_limited = true;
                    break 'related;
                }
                work_used += 1;
                inspected_edges += 1;
                let other = match direction {
                    Direction::Incoming => edge.source(),
                    Direction::Outgoing => edge.target(),
                };
                let relationship = edge.weight().clone();
                if other == *seed || graph.node(other).path == graph.node(*seed).path {
                    continue;
                }
                let reachability = if direction == Direction::Incoming
                    && !relationship.ambiguous
                    && relationship.confidence != ConfidenceTier::Low
                {
                    ImpactReachability::Direct
                } else {
                    ImpactReachability::Inferred
                };
                related_relationships
                    .entry(relationship.id.clone())
                    .or_insert_with(|| graph_impact_relationship(&relationship, reachability, 1));
                if direction == Direction::Outgoing {
                    related_targets.entry(other).or_insert(ImpactWalkTarget {
                        index: other,
                        depth: 1,
                        reachability,
                        relationship_path: vec![relationship],
                    });
                }
            }
        }
    }

    let mut targets = Vec::new();
    let mut depth_limited = false;
    'walk: while let Some((current, depth, relationship_path, reachability)) = queue.pop_front() {
        if depth >= MAX_IMPACT_DEPTH {
            depth_limited = true;
            continue;
        }
        let mut adjacent = graph
            .graph
            .edges_directed(current, Direction::Incoming)
            .map(|edge| (edge.source(), edge.weight().clone()))
            .collect::<Vec<_>>();
        adjacent.sort_by(|(left_index, left), (right_index, right)| {
            confidence_rank(right.confidence)
                .cmp(&confidence_rank(left.confidence))
                .then_with(|| left.ambiguous.cmp(&right.ambiguous))
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.id.cmp(&right.id))
                .then_with(|| graph.node(*left_index).id.cmp(&graph.node(*right_index).id))
        });
        for (other, relationship) in adjacent {
            if work_used >= MAX_IMPACT_WORK {
                work_limited = true;
                break 'walk;
            }
            work_used += 1;
            inspected_edges += 1;
            if !visited.insert(other) {
                continue;
            }
            let next_depth = depth + 1;
            let next_reachability = if relationship.ambiguous
                || relationship.confidence == ConfidenceTier::Low
                || reachability == ImpactReachability::Inferred
            {
                ImpactReachability::Inferred
            } else if next_depth == 1 {
                ImpactReachability::Direct
            } else {
                ImpactReachability::Transitive
            };
            let mut next_path = relationship_path.clone();
            next_path.push(relationship);
            targets.push(ImpactWalkTarget {
                index: other,
                depth: next_depth,
                reachability: next_reachability,
                relationship_path: next_path.clone(),
            });
            queue.push_back((other, next_depth, next_path, next_reachability));
        }
    }

    let omitted_nodes = usize::from(work_limited || depth_limited);
    ImpactWalk {
        seeds,
        seed_inputs,
        targets,
        related_targets: related_targets.into_values().collect(),
        related_relationships: related_relationships.into_values().collect(),
        bounds: ImpactTraversalBounds {
            max_depth: MAX_IMPACT_DEPTH,
            work_limit: MAX_IMPACT_WORK,
            work_used,
            visited_nodes: visited.len(),
            inspected_edges,
            seed_nodes: seed_nodes.len(),
            affected_nodes: visited.len().saturating_sub(seed_nodes.len()),
            omitted_nodes,
            truncated: omitted_nodes > 0,
            work_limited,
            depth_limited,
        },
        unresolved,
    }
}

fn impact_seed_inputs(
    graph: &RelationshipGraph, request: &ContextRequest, resolution: &ChangeResolution,
) -> Vec<ImpactSeedInput> {
    let mut inputs = BTreeMap::<(ImpactSeedKind, String, Option<String>), ImpactSeedInput>::new();
    let add = |inputs: &mut BTreeMap<_, _>, kind, path: String, symbol: Option<String>, reason: String| {
        let nodes = match &symbol {
            Some(symbol) => graph
                .symbol_nodes(symbol, None)
                .into_iter()
                .filter(|index| graph.node(*index).path == path || path == symbol.as_str())
                .collect::<Vec<_>>(),
            None => graph.file_nodes_for_target(&path),
        };
        let key = (kind, path.clone(), symbol.clone());
        let entry = inputs
            .entry(key)
            .or_insert(ImpactSeedInput { kind, path, symbol, reason, nodes: Vec::new() });
        entry.nodes.extend(nodes);
        entry.nodes.sort_by_key(|index| graph.node(*index).id);
        entry.nodes.dedup();
    };

    for change in &resolution.changes {
        add(
            &mut inputs,
            ImpactSeedKind::Change,
            change.path.clone(),
            None,
            format!("the local {} change resolved this path", change.kind.label()),
        );
        for symbol in &change.symbols {
            add(
                &mut inputs,
                ImpactSeedKind::Change,
                change.path.clone(),
                Some(symbol.name.clone()),
                format!("the local change overlaps the `{}` symbol", symbol.name),
            );
        }
    }
    for path in &request.paths {
        add(
            &mut inputs,
            ImpactSeedKind::File,
            normalize_seed_path(path),
            None,
            "the request supplied this file as an impact seed".to_owned(),
        );
    }
    for symbol in &request.symbols {
        add(
            &mut inputs,
            ImpactSeedKind::Symbol,
            symbol.clone(),
            Some(symbol.clone()),
            "the request supplied this symbol as an impact seed".to_owned(),
        );
    }
    for change in &request.changes {
        match change {
            TaskChangeSeed::Path(path) => add(
                &mut inputs,
                ImpactSeedKind::File,
                normalize_seed_path(path),
                None,
                "the request supplied this changed path as an impact seed".to_owned(),
            ),
            TaskChangeSeed::Symbol(symbol) => add(
                &mut inputs,
                ImpactSeedKind::Symbol,
                symbol.clone(),
                Some(symbol.clone()),
                "the request supplied this changed symbol as an impact seed".to_owned(),
            ),
        }
    }
    inputs.into_values().collect()
}

fn normalize_seed_path(path: &str) -> String {
    path.trim().replace('\\', "/").trim_start_matches("./").to_owned()
}

fn likely_tests(
    map: &MapReport, changed_paths: &BTreeSet<String>, source_by_path: &BTreeMap<&str, &ReadingSourceEvidence>,
    rank_by_path: &BTreeMap<&str, &FileRank>, candidates: &mut BTreeMap<String, TargetCandidate>,
    relationships: &mut Vec<ImpactRelationship>,
) -> Vec<ContextTest> {
    let mut tests = Vec::new();
    for source in source_by_path.values() {
        let Some(root) = map
            .landmarks
            .iter()
            .find(|landmark| landmark.kind == LandmarkKind::TestRoot && path_under(&source.path, &landmark.path))
        else {
            continue;
        };
        let shares_root = changed_paths
            .iter()
            .any(|path| same_project_root(map, path, &source.path));
        if !shares_root {
            continue;
        }
        let confidence = ConfidenceTier::Low;
        let reason = format!(
            "the path is under recognized test root `{}` in the changed project",
            root.path
        );
        add_target(
            candidates,
            TargetInput {
                path: source.path.clone(),
                source: (*source).clone(),
                rank: rank_by_path.get(source.path.as_str()).copied().cloned(),
                evidence: ImpactEvidenceKind::Structural,
                priority: 1_000_000_000,
                confidence,
                reason: reason.clone(),
                changed_symbols: None,
                impact_symbol: None,
                reachability: ImpactReachability::Inferred,
                depth: 0,
                relationship_path: Vec::new(),
                project_root: project_root_for_path(map, &source.path),
            },
        );
        if let Some(changed) = changed_paths
            .iter()
            .find(|path| same_project_root(map, path, &source.path))
        {
            relationships.push(ImpactRelationship {
                source: changed.clone(),
                target: source.path.clone(),
                evidence: ImpactEvidenceKind::Structural,
                confidence,
                reason: "both paths are in the same project root, and the target is under a recognized test root"
                    .to_owned(),
                symbol: None,
                ambiguous: false,
                reachability: ImpactReachability::Inferred,
                depth: 0,
                relationship_id: None,
            });
        }
        add_impact_test(
            &mut tests,
            ContextTest {
                path: source.path.clone(),
                reason,
                confidence,
                score: Some(1_000_000_000),
                depth: Some(0),
                reachability: Some(ImpactReachability::Inferred),
            },
        );
        if tests.len() == MAX_TESTS {
            break;
        }
    }
    tests
}

fn add_impact_test(tests: &mut Vec<ContextTest>, test: ContextTest) {
    if let Some(existing) = tests.iter_mut().find(|existing| existing.path == test.path) {
        let existing_score = existing.score.unwrap_or_default();
        let test_score = test.score.unwrap_or_default();
        if test_score > existing_score {
            *existing = test;
        }
        return;
    }
    if tests.len() < MAX_TESTS {
        tests.push(test);
        return;
    }
    let Some((index, weakest)) = tests.iter().enumerate().min_by(|(_, left), (_, right)| {
        left.score
            .unwrap_or_default()
            .cmp(&right.score.unwrap_or_default())
            .then_with(|| left.path.cmp(&right.path))
    }) else {
        return;
    };
    if test.score.unwrap_or_default() > weakest.score.unwrap_or_default() {
        tests[index] = test;
    }
    tests.sort_by(|left, right| {
        right
            .score
            .unwrap_or_default()
            .cmp(&left.score.unwrap_or_default())
            .then_with(|| left.path.cmp(&right.path))
    });
}

fn ownership_signals(map: &MapReport, changed_paths: &BTreeSet<String>) -> Vec<ImpactOwnershipSignal> {
    map.landmarks
        .iter()
        .filter(|landmark| landmark.kind == LandmarkKind::Ownership)
        .filter(|landmark| changed_paths.iter().any(|path| same_project_root(map, path, &landmark.path)))
        .take(MAX_OWNERSHIP_SIGNALS)
        .map(|landmark| ImpactOwnershipSignal {
            path: landmark.path.clone(),
            confidence: ConfidenceTier::Medium,
            reason: "recognized ownership configuration in the changed project; its rules were not semantically matched to individual paths".to_owned(),
        })
        .collect()
}

fn history_evidence(
    history: &HistoryReport, changed_paths: &BTreeSet<String>, relationships: &mut Vec<ImpactRelationship>,
) -> Vec<ImpactHistoryEvidence> {
    let mut evidence = Vec::new();
    for path in changed_paths {
        let churn = history
            .churn
            .as_ref()
            .and_then(|report| report.paths.iter().find(|candidate| candidate.path == *path));
        let bugs = history
            .bugs
            .as_ref()
            .and_then(|report| report.overlap_paths.iter().find(|candidate| candidate.path == *path));
        let commits = history
            .firefighting
            .as_ref()
            .map(|report| {
                report
                    .commits
                    .iter()
                    .filter(|commit| commit.paths.iter().any(|candidate| candidate == path))
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let Some(reason) = churn
            .map(|count| {
                format!(
                    "bounded churn history records {} commit(s) touching this changed path",
                    count.commits
                )
            })
            .or_else(|| {
                bugs.map(|count| {
                    format!(
                        "bounded bug-history overlap records {} commit(s) touching this changed path",
                        count.commits
                    )
                })
            })
            .or_else(|| {
                (!commits.is_empty())
                    .then(|| "bounded firefighting-language commits touched this changed path".to_owned())
            })
        else {
            continue;
        };
        relationships.push(ImpactRelationship {
            source: "history".to_owned(),
            target: path.clone(),
            evidence: ImpactEvidenceKind::History,
            confidence: ConfidenceTier::Medium,
            reason: reason.clone(),
            symbol: None,
            ambiguous: false,
            reachability: ImpactReachability::Direct,
            depth: 0,
            relationship_id: None,
        });
        evidence.push(ImpactHistoryEvidence {
            path: path.clone(),
            evidence: ImpactEvidenceKind::History,
            confidence: ConfidenceTier::Medium,
            reason,
            commits,
        });
        if evidence.len() == MAX_HISTORY_EVIDENCE {
            break;
        }
    }
    evidence
}

fn project_summaries(targets: &[ImpactTarget], tests: &[ContextTest]) -> Vec<ImpactProject> {
    #[derive(Default)]
    struct Accumulator {
        score: u64,
        confidence: ConfidenceTier,
        reachability: ImpactReachability,
        paths: BTreeSet<String>,
        symbols: BTreeSet<String>,
        tests: BTreeSet<String>,
    }
    let mut projects = BTreeMap::<String, Accumulator>::new();
    for target in targets {
        let Some(project) = target.project_root.clone() else {
            continue;
        };
        let entry = projects.entry(project).or_default();
        entry.score = entry.score.max(target.score);
        entry.confidence = entry.confidence.max(target.confidence);
        if entry.paths.is_empty()
            || impact_reachability_rank(target.reachability) > impact_reachability_rank(entry.reachability)
        {
            entry.reachability = target.reachability;
        }
        entry.paths.insert(target.path.clone());
        entry
            .symbols
            .extend(target.symbols.iter().map(|symbol| symbol.symbol.name.clone()));
    }
    for test in tests {
        for project in projects.values_mut() {
            if project.paths.contains(&test.path) {
                project.tests.insert(test.path.clone());
            }
        }
    }
    let mut result = projects
        .into_iter()
        .map(|(path, project)| ImpactProject {
            path,
            score: project.score,
            confidence: project.confidence,
            reachability: project.reachability,
            affected_paths: project.paths.into_iter().collect(),
            affected_symbols: project.symbols.into_iter().collect(),
            affected_tests: project.tests.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| right.score.cmp(&left.score).then_with(|| left.path.cmp(&right.path)));
    result.truncate(MAX_IMPACT_PROJECTS);
    result
}

fn uncertainty(
    map: &MapReport, history_complete: bool, resolution: &ChangeResolution, bounds: &ImpactTraversalBounds,
    unresolved: &[String],
) -> Vec<ContextUncertainty> {
    let mut uncertainty = resolution
        .uncertainty
        .iter()
        .map(|item| ContextUncertainty { kind: format!("change_{}", item.kind), detail: item.detail.clone() })
        .collect::<Vec<_>>();
    uncertainty.extend(
        unresolved
            .iter()
            .map(|detail| ContextUncertainty { kind: "unresolved_seed".to_owned(), detail: detail.clone() }),
    );
    if bounds.work_limited {
        uncertainty.push(ContextUncertainty {
            kind: "impact_work_limit".to_owned(),
            detail: format!(
                "Downstream impact traversal inspected {} edges and stopped at its {}-edge work limit; the result is incomplete.",
                bounds.inspected_edges, bounds.work_limit
            ),
        });
    }
    if bounds.depth_limited {
        uncertainty.push(ContextUncertainty {
            kind: "impact_depth_limit".to_owned(),
            detail: format!(
                "Downstream impact traversal reached its maximum depth of {}; deeper affected nodes were not inspected.",
                bounds.max_depth
            ),
        });
    }
    uncertainty.push(ContextUncertainty {
        kind: "evidence_scope".to_owned(),
        detail: "Impact relationships are bounded lexical, graph, structural, manifest, and history evidence. They do not establish semantic callers, callees, ownership matches, or breakage.".to_owned(),
    });
    if map.collections.edges.truncated || map.collections.ranking.truncated {
        uncertainty.push(ContextUncertainty {
            kind: "bounded_evidence".to_owned(),
            detail: "Some graph or ranking candidates were omitted by the active profile or resource limits."
                .to_owned(),
        });
    }
    if !history_complete {
        uncertainty.push(ContextUncertainty {
            kind: "incomplete_history".to_owned(),
            detail: "History evidence is incomplete for the selected repository state.".to_owned(),
        });
    }
    uncertainty.into_iter().take(MAX_UNCERTAINTIES).collect()
}

fn project_root_for_path(map: &MapReport, path: &str) -> Option<String> {
    map.project_roots
        .iter()
        .filter(|root| root.path == "." || path_under(path, &root.path))
        .max_by_key(|root| root.path.len())
        .map(|root| root.path.clone())
}

fn same_project_root(map: &MapReport, left: &str, right: &str) -> bool {
    project_root_for_path(map, left) == project_root_for_path(map, right)
}

fn path_under(path: &str, root: &str) -> bool {
    root == "." || path == root || path.strip_prefix(root).is_some_and(|suffix| suffix.starts_with('/'))
}

fn is_test_path(path: &str) -> bool {
    path.starts_with("test/")
        || path.starts_with("tests/")
        || path.contains("/test/")
        || path.contains("/tests/")
        || path.ends_with("_test.go")
        || path.ends_with("_test.rs")
        || path.ends_with(".test.js")
        || path.ends_with(".test.ts")
        || path.ends_with(".spec.js")
        || path.ends_with(".spec.ts")
}

fn confidence_rank(confidence: ConfidenceTier) -> u8 {
    match confidence {
        ConfidenceTier::High => 3,
        ConfidenceTier::Medium => 2,
        ConfidenceTier::Low => 1,
    }
}

fn add_if_fits(report: &mut ImpactReport, budget: usize, add: impl FnOnce(&mut ImpactReport)) -> bool {
    let before = report.clone();
    add(report);
    if estimate_tokens(report) > budget {
        *report = before;
        return false;
    }
    true
}

fn estimate_tokens(report: &ImpactReport) -> usize {
    let mut selected = report.clone();
    selected.budget.estimated_tokens = 0;
    serde_json::to_string(&selected).map_or(usize::MAX, |json| token_count(&json))
}
