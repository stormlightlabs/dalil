use std::collections::{BTreeSet, VecDeque};

use petgraph::Direction;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;

use super::relationships::RelationshipGraph;
use super::*;

const MAX_DEPTH: usize = 64;
const MAX_WORK_LIMIT: usize = 100_000;
const MAX_RESULT_LIMIT: usize = 256;

#[derive(Clone)]
struct AdjacentEdge {
    other: NodeIndex,
    relationship: RepositoryRelationship,
}

#[derive(Clone)]
struct TraversalCandidate {
    node: TraversalNode,
}

struct NeighborhoodWalk {
    candidates: Vec<TraversalCandidate>,
    total: usize,
    visited_nodes: usize,
    inspected_edges: usize,
    work_used: usize,
    depth_reached: bool,
    work_limited: bool,
    project_omissions: usize,
}

struct PathSearch {
    path: Option<TraversalPath>,
    visited_nodes: usize,
    inspected_edges: usize,
    work_used: usize,
    depth_reached: bool,
    work_limited: bool,
    project_omissions: usize,
}

#[derive(Clone)]
struct PathState {
    index: NodeIndex,
    depth: usize,
    nodes: Vec<NodeIndex>,
    relationships: Vec<RepositoryRelationship>,
}

pub(crate) fn compile(mut request: TraversalRequest, map: &MapReport) -> TraversalResults {
    normalize_request(&mut request);
    let graph = RelationshipGraph::build(map);
    let project = request.project.as_deref();
    let file_only = request.operation == TraversalOperation::ReverseDependencies;
    let starts = graph
        .nodes_for_target(&request.start)
        .into_iter()
        .filter(|index| {
            let node = graph.node(*index);
            (!file_only || node.kind == RelationshipNodeKind::File) && project_matches(&node, project)
        })
        .collect::<Vec<_>>();
    let kinds = effective_kinds(&request);

    let (mut nodes, mut relationships, paths, mut bounds, mut omissions) = match request.operation {
        TraversalOperation::Neighbors | TraversalOperation::ReverseDependencies => {
            let walk = walk_neighborhood(&graph, &starts, &request, &kinds, file_only);
            let total = walk.total;
            let candidates = walk.candidates;
            let (nodes, budget_stopped) = select_nodes_by_budget(&candidates, request.budget);
            let relationships = relationships_for_nodes(&nodes);
            let mut omissions = Vec::new();
            if total > request.result_limit {
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::ResultLimit,
                    count: total - request.result_limit,
                    detail: "the result limit ended the traversal result".to_owned(),
                });
            }
            if budget_stopped {
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::TokenBudget,
                    count: total.saturating_sub(nodes.len()),
                    detail: "the token budget ended the traversal result".to_owned(),
                });
            }
            if walk.work_limited {
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::WorkLimit,
                    count: 1,
                    detail: "the traversal work limit stopped edge inspection".to_owned(),
                });
            }
            if walk.depth_reached {
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::DepthLimit,
                    count: 1,
                    detail: format!("the maximum traversal depth of {} was reached", request.max_depth),
                });
            }
            if walk.project_omissions > 0 {
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::ProjectBoundary,
                    count: walk.project_omissions,
                    detail: "edges to nodes outside the selected project were not traversed".to_owned(),
                });
            }
            (
                nodes,
                relationships,
                Vec::new(),
                TraversalBounds {
                    token_budget: request.budget,
                    result_limit: request.result_limit,
                    max_depth: request.max_depth,
                    work_limit: request.work_limit,
                    work_used: walk.work_used,
                    visited_nodes: walk.visited_nodes,
                    inspected_edges: walk.inspected_edges,
                    total,
                    returned: 0,
                    omitted: 0,
                    returned_relationships: 0,
                    truncated: false,
                    work_limited: walk.work_limited,
                    depth_limited: walk.depth_reached,
                    found_path: false,
                },
                omissions,
            )
        }
        TraversalOperation::Path => {
            let targets = request
                .target
                .as_deref()
                .map(|target| {
                    graph
                        .nodes_for_target(target)
                        .into_iter()
                        .filter(|index| project_matches(&graph.node(*index), project))
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            let search = find_path(&graph, &starts, &targets, &request, &kinds);
            let mut omissions = Vec::new();
            if search.work_limited {
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::WorkLimit,
                    count: 1,
                    detail: "the traversal work limit stopped edge inspection".to_owned(),
                });
            }
            if search.depth_reached {
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::DepthLimit,
                    count: 1,
                    detail: format!("the maximum path depth of {} was reached", request.max_depth),
                });
            }
            if search.project_omissions > 0 {
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::ProjectBoundary,
                    count: search.project_omissions,
                    detail: "edges to nodes outside the selected project were not traversed".to_owned(),
                });
            }
            if search.path.is_none() {
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::NoPath,
                    count: 1,
                    detail: "no path connected the requested anchors within the traversal bounds".to_owned(),
                });
            }
            let mut paths = search.path.into_iter().collect::<Vec<_>>();
            let budget_stopped = paths.first().is_some_and(|path| {
                estimate_payload(&[], &path.relationships, std::slice::from_ref(path)) > request.budget
            });
            if budget_stopped {
                paths.clear();
                omissions.push(TraversalOmission {
                    reason: TraversalOmissionReason::TokenBudget,
                    count: 1,
                    detail: "the token budget could not fit the path evidence".to_owned(),
                });
            }
            let total = usize::from(!paths.is_empty());
            (
                Vec::new(),
                paths.iter().flat_map(|path| path.relationships.clone()).collect(),
                paths,
                TraversalBounds {
                    token_budget: request.budget,
                    result_limit: request.result_limit,
                    max_depth: request.max_depth,
                    work_limit: request.work_limit,
                    work_used: search.work_used,
                    visited_nodes: search.visited_nodes,
                    inspected_edges: search.inspected_edges,
                    total,
                    returned: total,
                    omitted: 0,
                    returned_relationships: 0,
                    truncated: false,
                    work_limited: search.work_limited,
                    depth_limited: search.depth_reached,
                    found_path: total > 0,
                },
                omissions,
            )
        }
    };

    let budget_stopped = omissions
        .iter()
        .any(|omission| omission.reason == TraversalOmissionReason::TokenBudget);
    let returned_relationships = relationships.len();
    let returned = if request.operation == TraversalOperation::Path { paths.len() } else { nodes.len() };
    let total = bounds.total;
    bounds.returned = returned;
    bounds.omitted = total.saturating_sub(returned);
    bounds.returned_relationships = returned_relationships;
    bounds.truncated = bounds.omitted > 0 || bounds.work_limited || bounds.depth_limited || budget_stopped;
    bounds.found_path = !paths.is_empty();
    relationships.sort_by(|left, right| left.id.cmp(&right.id));
    relationships.dedup_by(|left, right| left.id == right.id);
    nodes.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| left.node.path.cmp(&right.node.path))
            .then_with(|| left.node.id.cmp(&right.node.id))
    });

    if !map.omissions.is_empty() {
        omissions.push(TraversalOmission {
            reason: TraversalOmissionReason::SourceEvidence,
            count: map.omissions.len(),
            detail: "some repository paths were omitted before graph construction".to_owned(),
        });
    }
    omissions.sort_by(|left, right| left.reason.cmp(&right.reason));

    let mut limitations = map.limitations.clone();
    limitations.push(
        "Traversal follows retained lexical relationship edges; compiler, type, and runtime resolution is not performed."
            .to_owned(),
    );
    match request.operation {
        TraversalOperation::Neighbors => {
            limitations.push("Neighbor depth is measured in graph edges from each start node.".to_owned());
        }
        TraversalOperation::Path => {
            limitations.push(
                "The returned path is the shortest path found; confidence and edge ID order break ties.".to_owned(),
            );
        }
        TraversalOperation::ReverseDependencies => {
            limitations.push("Reverse dependencies follow incoming file-level dependency and import edges.".to_owned());
        }
    }
    limitations.sort();
    limitations.dedup();

    let source_files = graph
        .file_nodes
        .values()
        .filter_map(|index| graph.graph.node_weight(*index))
        .count();
    let symbols = graph
        .graph
        .node_weights()
        .filter(|node| node.kind == RelationshipNodeKind::Symbol)
        .count();
    let partial = map.availability.resource_limited
        || map.availability.unsupported_paths > 0
        || !map.omissions.is_empty()
        || graph.graph.node_weights().any(|node| node.partial);
    let provenance = RelationshipProvenance {
        repository: map.repository_root.clone(),
        scope_path: map.scope_path.clone(),
        profile: request.profile,
        head: map.head.clone(),
        worktree: map.worktree.clone(),
        cache: super::relationships::cache_provenance(map),
        query_packs: map.query_packs.clone(),
        source_files: CollectionSummary::complete(source_files),
        symbols: CollectionSummary::complete(symbols),
        relationships: CollectionSummary::complete(graph.graph.edge_count()),
        partial,
        limitations: limitations.clone(),
    };
    TraversalResults { request, nodes, relationships, paths, bounds, omissions, provenance, limitations }
}

fn normalize_request(request: &mut TraversalRequest) {
    request.repository = request.repository.trim().to_owned();
    request.start = request.start.trim().replace('\\', "/");
    request.target = request.target.take().map(|target| target.trim().replace('\\', "/"));
    request.project = request
        .project
        .take()
        .map(|project| project.trim().replace('\\', "/").trim_start_matches("./").to_owned());
    request.max_depth = request.max_depth.min(MAX_DEPTH);
    request.work_limit = if request.work_limit == 0 { 1 } else { request.work_limit.min(MAX_WORK_LIMIT) };
    request.result_limit = if request.result_limit == 0 { 1 } else { request.result_limit.min(MAX_RESULT_LIMIT) };
    request.budget = request.budget.max(1);
    request.relationship_kinds.sort();
    request.relationship_kinds.dedup();
    if request.operation == TraversalOperation::ReverseDependencies {
        request.direction = TraversalDirection::Incoming;
        if request.relationship_kinds.is_empty() {
            request.relationship_kinds = vec![
                RepositoryRelationshipKind::Dependency,
                RepositoryRelationshipKind::Import,
            ];
        }
    }
}

fn effective_kinds(request: &TraversalRequest) -> Vec<RepositoryRelationshipKind> {
    if request.relationship_kinds.is_empty() {
        vec![
            RepositoryRelationshipKind::Dependency,
            RepositoryRelationshipKind::Import,
            RepositoryRelationshipKind::Reference,
            RepositoryRelationshipKind::TypeReference,
            RepositoryRelationshipKind::Call,
        ]
    } else {
        request.relationship_kinds.clone()
    }
}

fn walk_neighborhood(
    graph: &RelationshipGraph, starts: &[NodeIndex], request: &TraversalRequest, kinds: &[RepositoryRelationshipKind],
    file_only: bool,
) -> NeighborhoodWalk {
    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::new();
    for start in starts {
        if visited.insert(*start) {
            queue.push_back((*start, 0));
        }
    }

    let mut candidates = Vec::new();
    let mut total = 0;
    let mut work_used = 0;
    let mut inspected_edges = 0;
    let mut depth_reached = false;
    let mut work_limited = false;
    let mut project_omissions = 0;

    'walk: while let Some((current, depth)) = queue.pop_front() {
        if depth >= request.max_depth {
            depth_reached = true;
            continue;
        }
        for adjacent in adjacent_edges(graph, current, request.direction, kinds, file_only) {
            if work_used >= request.work_limit {
                work_limited = true;
                break 'walk;
            }
            work_used += 1;
            inspected_edges += 1;
            let node = graph.node(adjacent.other);
            if !project_matches(&node, request.project.as_deref()) {
                project_omissions += 1;
                continue;
            }
            if !visited.insert(adjacent.other) {
                continue;
            }
            let next_depth = depth + 1;
            total += 1;
            if candidates.len() < request.result_limit {
                candidates.push(TraversalCandidate {
                    node: TraversalNode { node, depth: next_depth, via: Some(adjacent.relationship) },
                });
            }
            queue.push_back((adjacent.other, next_depth));
        }
    }

    NeighborhoodWalk {
        candidates,
        total,
        visited_nodes: visited.len(),
        inspected_edges,
        work_used,
        depth_reached,
        work_limited,
        project_omissions,
    }
}

fn find_path(
    graph: &RelationshipGraph, starts: &[NodeIndex], targets: &BTreeSet<NodeIndex>, request: &TraversalRequest,
    kinds: &[RepositoryRelationshipKind],
) -> PathSearch {
    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::new();
    for start in starts {
        if visited.insert(*start) {
            queue.push_back(PathState { index: *start, depth: 0, nodes: vec![*start], relationships: Vec::new() });
        }
    }

    let mut work_used = 0;
    let mut inspected_edges = 0;
    let mut depth_reached = false;
    let mut work_limited = false;
    let mut project_omissions = 0;

    while let Some(state) = queue.pop_front() {
        if targets.contains(&state.index) {
            return PathSearch {
                path: Some(path_from_state(graph, &state)),
                visited_nodes: visited.len(),
                inspected_edges,
                work_used,
                depth_reached,
                work_limited,
                project_omissions,
            };
        }
        if state.depth >= request.max_depth {
            depth_reached = true;
            continue;
        }
        for adjacent in adjacent_edges(graph, state.index, request.direction, kinds, false) {
            if work_used >= request.work_limit {
                work_limited = true;
                break;
            }
            work_used += 1;
            inspected_edges += 1;
            let node = graph.node(adjacent.other);
            if !project_matches(&node, request.project.as_deref()) {
                project_omissions += 1;
                continue;
            }
            if !visited.insert(adjacent.other) {
                continue;
            }
            let mut nodes = state.nodes.clone();
            nodes.push(adjacent.other);
            let mut relationships = state.relationships.clone();
            relationships.push(adjacent.relationship);
            queue.push_back(PathState { index: adjacent.other, depth: state.depth + 1, nodes, relationships });
        }
        if work_limited {
            break;
        }
    }

    PathSearch {
        path: None,
        visited_nodes: visited.len(),
        inspected_edges,
        work_used,
        depth_reached,
        work_limited,
        project_omissions,
    }
}

fn path_from_state(graph: &RelationshipGraph, state: &PathState) -> TraversalPath {
    let mut limitations = state
        .nodes
        .iter()
        .flat_map(|index| graph.node(*index).limitations)
        .chain(
            state
                .relationships
                .iter()
                .flat_map(|relationship| relationship.limitations.clone()),
        )
        .collect::<Vec<_>>();
    limitations.sort();
    limitations.dedup();
    TraversalPath {
        nodes: state.nodes.iter().map(|index| graph.node(*index)).collect(),
        relationships: state.relationships.clone(),
        depth: state.depth,
        confidence: state
            .relationships
            .iter()
            .map(|relationship| relationship.confidence)
            .min()
            .unwrap_or(ConfidenceTier::High),
        ambiguous: state.relationships.iter().any(|relationship| relationship.ambiguous),
        limitations,
    }
}

fn adjacent_edges(
    graph: &RelationshipGraph, index: NodeIndex, direction: TraversalDirection, kinds: &[RepositoryRelationshipKind],
    file_only: bool,
) -> Vec<AdjacentEdge> {
    let mut edges = Vec::new();
    let mut seen = BTreeSet::new();
    let directions = match direction {
        TraversalDirection::Incoming => vec![Direction::Incoming],
        TraversalDirection::Outgoing => vec![Direction::Outgoing],
        TraversalDirection::Both => vec![Direction::Outgoing, Direction::Incoming],
    };
    for direction in directions {
        for edge in graph.graph.edges_directed(index, direction) {
            let relationship = edge.weight();
            if !kinds.contains(&relationship.kind) || !seen.insert(relationship.id.clone()) {
                continue;
            }
            let other = match direction {
                Direction::Outgoing => edge.target(),
                Direction::Incoming => edge.source(),
            };
            if file_only
                && (graph.node(index).kind != RelationshipNodeKind::File
                    || graph.node(other).kind != RelationshipNodeKind::File)
            {
                continue;
            }
            edges.push(AdjacentEdge { other, relationship: relationship.clone() });
        }
    }
    edges.sort_by(|left, right| {
        confidence_rank(right.relationship.confidence)
            .cmp(&confidence_rank(left.relationship.confidence))
            .then_with(|| left.relationship.ambiguous.cmp(&right.relationship.ambiguous))
            .then_with(|| left.relationship.kind.cmp(&right.relationship.kind))
            .then_with(|| left.relationship.id.cmp(&right.relationship.id))
            .then_with(|| graph.node(left.other).id.cmp(&graph.node(right.other).id))
    });
    edges
}

fn relationships_for_nodes(nodes: &[TraversalNode]) -> Vec<RepositoryRelationship> {
    let mut relationships = nodes.iter().filter_map(|node| node.via.clone()).collect::<Vec<_>>();
    relationships.sort_by(|left, right| left.id.cmp(&right.id));
    relationships.dedup_by(|left, right| left.id == right.id);
    relationships
}

fn select_nodes_by_budget(candidates: &[TraversalCandidate], budget: usize) -> (Vec<TraversalNode>, bool) {
    let mut selected = Vec::new();
    let mut budget_stopped = false;
    for candidate in candidates {
        let mut proposed = selected.clone();
        proposed.push(candidate.node.clone());
        let relationships = relationships_for_nodes(&proposed);
        if estimate_payload(&proposed, &relationships, &[]) > budget {
            budget_stopped = true;
            break;
        }
        selected.push(candidate.node.clone());
    }
    (selected, budget_stopped)
}

fn estimate_payload(
    nodes: &[TraversalNode], relationships: &[RepositoryRelationship], paths: &[TraversalPath],
) -> usize {
    serde_json::to_string(&(nodes, relationships, paths)).map_or(usize::MAX, |payload| token_count(&payload))
}

fn project_matches(node: &RelationshipNode, project: Option<&str>) -> bool {
    project.is_none_or(|project| node.project_root.as_deref().unwrap_or(".") == project)
}

fn confidence_rank(confidence: ConfidenceTier) -> u8 {
    match confidence {
        ConfidenceTier::High => 3,
        ConfidenceTier::Medium => 2,
        ConfidenceTier::Low => 1,
    }
}
