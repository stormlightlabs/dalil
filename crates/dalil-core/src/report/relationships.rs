use std::collections::{BTreeMap, BTreeSet};

use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::visit::EdgeRef;

use super::*;

const DEFAULT_RESULT_LIMIT: usize = 20;
const MAX_RESULT_LIMIT: usize = 256;

#[derive(Clone)]
struct RelationshipSource {
    path: String,
    language: SourceLanguage,
    status: FileAnalysisStatus,
    symbols: Vec<SourceSymbol>,
    limitations: Vec<String>,
    project_root: Option<String>,
}

struct RelationshipGraph {
    graph: StableDiGraph<RelationshipNode, RepositoryRelationship>,
    file_nodes: BTreeMap<String, NodeIndex>,
}

#[derive(Clone)]
struct Candidate {
    node: RelationshipNode,
    relation: RelationshipMatchKind,
    edge: Option<RepositoryRelationship>,
    reason: String,
    evidence: Vec<RelationshipEvidence>,
    confidence: ConfidenceTier,
    ambiguous: bool,
    limitations: Vec<String>,
}

pub(crate) fn compile(mut request: RelationshipRequest, map: &MapReport) -> RelationshipResults {
    normalize_request(&mut request);
    let graph = RelationshipGraph::build(map);
    let mut candidates = candidates_for(&graph, &request);
    candidates.sort_by(candidate_order);

    let total = candidates.len();
    let offset = request.offset.min(total);
    let page_end = offset.saturating_add(request.result_limit).min(total);
    let mut selected = Vec::new();
    let mut budget_stopped = false;
    for candidate in candidates.iter().skip(offset).take(request.result_limit) {
        let next = candidate_to_match(candidate);
        let mut proposed = selected.iter().map(candidate_to_match).collect::<Vec<_>>();
        proposed.push(next);
        let edges = candidate_edges(&proposed, &candidates);
        if estimate_payload(&proposed, &edges) > request.budget {
            budget_stopped = true;
            break;
        }
        selected.push(candidate.clone());
    }

    let matches = selected.iter().map(candidate_to_match).collect::<Vec<_>>();
    let relationships = candidate_edges(&matches, &candidates);
    let estimated_tokens = estimate_payload(&matches, &relationships);
    let returned = matches.len();
    let omitted = total.saturating_sub(returned);
    let total_relationships = unique_relationships(&candidates).len();
    let returned_relationships = relationships.len();
    let continuation = (offset.saturating_add(returned) < total)
        .then_some(RelationshipCursor { offset: offset.saturating_add(returned), limit: request.result_limit });
    let mut omissions = Vec::new();
    if budget_stopped {
        omissions.push(RelationshipOmission {
            reason: RelationshipOmissionReason::TokenBudget,
            count: total.saturating_sub(offset.saturating_add(returned)),
            detail: "the token budget ended this relationship page".to_owned(),
        });
    } else if page_end < total {
        omissions.push(RelationshipOmission {
            reason: RelationshipOmissionReason::ResultLimit,
            count: total - page_end,
            detail: "the result limit ended this relationship page".to_owned(),
        });
    }
    if !map.omissions.is_empty() {
        omissions.push(RelationshipOmission {
            reason: RelationshipOmissionReason::SourceEvidence,
            count: map.omissions.len(),
            detail: "some repository paths were omitted before graph construction".to_owned(),
        });
    }

    let request_budget = request.budget;
    let request_result_limit = request.result_limit;
    let request_profile = request.profile;
    let operation = request.operation;
    let mut limitations = map.limitations.clone();
    limitations.extend(operation_limitations(operation));
    if operation == RelationshipOperation::Callers || operation == RelationshipOperation::Callees {
        limitations.push(
            "Callers and callees use syntax tagged as calls; an ambiguous lexical candidate is not a resolved semantic call relationship."
                .to_owned(),
        );
    }
    limitations.sort();
    limitations.dedup();

    let source_total = graph
        .file_nodes
        .values()
        .filter_map(|index| graph.graph.node_weight(*index))
        .count();
    let symbol_total = graph
        .graph
        .node_weights()
        .filter(|node| node.kind == RelationshipNodeKind::Symbol)
        .count();
    let relationship_total = graph.graph.edge_count();
    let partial = map.availability.resource_limited
        || map.availability.unsupported_paths > 0
        || !map.omissions.is_empty()
        || graph.graph.node_weights().any(|node| node.partial);
    let provenance_limitations = limitations.clone();
    let provenance = RelationshipProvenance {
        repository: map.repository_root.clone(),
        scope_path: map.scope_path.clone(),
        profile: request_profile,
        head: map.head.clone(),
        worktree: map.worktree.clone(),
        cache: cache_provenance(map),
        query_packs: map.query_packs.clone(),
        source_files: CollectionSummary::complete(source_total),
        symbols: CollectionSummary::complete(symbol_total),
        relationships: CollectionSummary::complete(relationship_total),
        partial,
        limitations: provenance_limitations,
    };
    RelationshipResults {
        request,
        matches,
        relationships,
        bounds: RelationshipBounds {
            token_budget: request_budget,
            result_limit: request_result_limit,
            offset,
            total,
            returned,
            omitted,
            total_relationships,
            returned_relationships,
            omitted_relationships: total_relationships.saturating_sub(returned_relationships),
            estimated_tokens,
            truncated: omitted > 0,
            continuation,
        },
        omissions,
        provenance,
        limitations,
    }
}

fn normalize_request(request: &mut RelationshipRequest) {
    request.repository = request.repository.trim().to_owned();
    request.target = request.target.trim().replace('\\', "/");
    request.result_limit = if request.result_limit == 0 {
        DEFAULT_RESULT_LIMIT
    } else {
        request.result_limit.min(MAX_RESULT_LIMIT)
    };
    request.budget = request.budget.max(1);
}

fn cache_provenance(map: &MapReport) -> CacheProvenance {
    CacheProvenance {
        mode: map.cache.mode,
        status: map.cache.status,
        index_status: map.cache.index_status,
        index_detail: map.cache.index_detail.clone(),
        available: map.cache.mode != CacheMode::Disabled,
        reused: map.cache.reused.len(),
        invalidated: map.cache.invalidated.len(),
        hits: map.cache.hits,
        misses: map.cache.misses,
        refreshed: map.cache.refreshed.len(),
        stale: map.cache.stale.len(),
    }
}

fn operation_limitations(operation: RelationshipOperation) -> Vec<String> {
    let common = "Relationship targets are derived from retained Tree-sitter symbols and lexical file edges; compiler, type, and runtime resolution is not performed.";
    let specific = match operation {
        RelationshipOperation::Symbol | RelationshipOperation::Definitions | RelationshipOperation::References => {
            "Definitions and references preserve syntax evidence, including duplicate names and bare references."
        }
        RelationshipOperation::Imports => {
            "Imports are returned only when literal import or module evidence contributed a lexical edge."
        }
        RelationshipOperation::Dependencies | RelationshipOperation::ReverseDependencies => {
            "Dependencies are file-level lexical relationships and can have multiple candidate targets."
        }
        RelationshipOperation::Tests => {
            "Related tests are test paths with retained lexical relationships to the requested file or symbol."
        }
        RelationshipOperation::Callers | RelationshipOperation::Callees => {
            "Call relationships require syntax evidence for a call expression; dynamic dispatch and unresolved names remain uncertain."
        }
    };
    vec![common.to_owned(), specific.to_owned()]
}

impl RelationshipGraph {
    fn build(map: &MapReport) -> Self {
        Self::from_parts(relationship_sources(map), relationship_edges(map))
    }

    fn from_parts(sources: Vec<RelationshipSource>, edges: Vec<LexicalEdge>) -> Self {
        let mut graph = StableDiGraph::new();
        let mut file_nodes = BTreeMap::new();
        let mut node_ids = BTreeMap::new();

        for source in &sources {
            let node = RelationshipNode {
                id: file_node_id(&source.path),
                kind: RelationshipNodeKind::File,
                path: source.path.clone(),
                project_root: source.project_root.clone(),
                language: Some(source.language),
                symbol: None,
                partial: source.status == FileAnalysisStatus::Partial,
                limitations: source.limitations.clone(),
            };
            let index = graph.add_node(node.clone());
            file_nodes.insert(source.path.clone(), index);
            node_ids.insert(node.id, index);
        }

        for source in &sources {
            for symbol in &source.symbols {
                let node = RelationshipNode {
                    id: symbol_node_id(&source.path, symbol),
                    kind: RelationshipNodeKind::Symbol,
                    path: source.path.clone(),
                    project_root: source.project_root.clone(),
                    language: Some(source.language),
                    symbol: Some(symbol.clone()),
                    partial: source.status == FileAnalysisStatus::Partial,
                    limitations: source.limitations.clone(),
                };
                let index = graph.add_node(node.clone());
                node_ids.insert(node.id, index);
            }
        }

        let source_by_path = sources
            .iter()
            .map(|source| (source.path.as_str(), source))
            .collect::<BTreeMap<_, _>>();
        let mut relationship_ids = BTreeSet::new();
        for edge in &edges {
            let (Some(source), Some(target)) = (
                source_by_path.get(edge.source.as_str()),
                source_by_path.get(edge.target.as_str()),
            ) else {
                continue;
            };
            let Some(&source_file) = file_nodes.get(&edge.source) else {
                continue;
            };
            let Some(&target_file) = file_nodes.get(&edge.target) else {
                continue;
            };
            let import = source.symbols.iter().any(|symbol| {
                symbol.role == SymbolRole::Definition
                    && symbol.evidence == SymbolEvidence::Import
                    && symbol.name == edge.symbol
            });
            let file_kind =
                if import { RepositoryRelationshipKind::Import } else { RepositoryRelationshipKind::Dependency };
            let file_relationship = relationship_for_edge(
                edge,
                graph.node_weight(source_file).expect("file node was inserted"),
                graph.node_weight(target_file).expect("file node was inserted"),
                file_kind,
                None,
            );
            if relationship_ids.insert(file_relationship.id.clone()) {
                graph.add_edge(source_file, target_file, file_relationship);
            }

            let source_symbols = source
                .symbols
                .iter()
                .filter(|symbol| {
                    (symbol.role == SymbolRole::Reference && symbol.name == edge.symbol)
                        || (symbol.role == SymbolRole::Definition
                            && symbol.evidence == SymbolEvidence::Import
                            && symbol.name == edge.symbol)
                })
                .filter_map(|symbol| node_ids.get(&symbol_node_id(&source.path, symbol)).copied())
                .collect::<Vec<_>>();
            let imported_names = source
                .symbols
                .iter()
                .filter(|symbol| {
                    symbol.role == SymbolRole::Definition
                        && symbol.evidence == SymbolEvidence::Import
                        && symbol.name == edge.symbol
                })
                .flat_map(|symbol| import_target_names(&symbol.name, &symbol.context))
                .collect::<BTreeSet<_>>();
            let target_symbols = target
                .symbols
                .iter()
                .filter(|symbol| {
                    is_graph_definition(symbol) && (symbol.name == edge.symbol || imported_names.contains(&symbol.name))
                })
                .filter_map(|symbol| node_ids.get(&symbol_node_id(&target.path, symbol)).copied())
                .collect::<Vec<_>>();
            for source_symbol in source_symbols {
                let source_endpoint = graph
                    .node_weight(source_symbol)
                    .and_then(|node| node.symbol.as_ref())
                    .and_then(|symbol| {
                        (symbol.evidence == SymbolEvidence::Call)
                            .then(|| call_owner_node(&source.path, symbol, &source.symbols, &node_ids))
                            .flatten()
                    })
                    .unwrap_or(source_symbol);
                for target_symbol in &target_symbols {
                    let source_node = graph.node_weight(source_endpoint).expect("source symbol was inserted");
                    let target_node = graph.node_weight(*target_symbol).expect("target symbol was inserted");
                    let kind = graph
                        .node_weight(source_symbol)
                        .and_then(|node| node.symbol.as_ref())
                        .map(|symbol| symbol_relationship_kind(symbol.evidence))
                        .unwrap_or(RepositoryRelationshipKind::Reference);
                    let relationship = relationship_for_edge(edge, source_node, target_node, kind, Some(&edge.symbol));
                    if relationship_ids.insert(relationship.id.clone()) {
                        graph.add_edge(source_endpoint, *target_symbol, relationship);
                    }
                }
            }
        }
        Self { graph, file_nodes }
    }

    fn node(&self, index: NodeIndex) -> RelationshipNode {
        self.graph
            .node_weight(index)
            .expect("relationship graph index points to a node")
            .clone()
    }

    fn symbol_nodes(&self, target: &str, role: Option<SymbolRole>) -> Vec<NodeIndex> {
        let mut nodes = self
            .graph
            .node_indices()
            .filter(|index| {
                let node = self.graph.node_weight(*index).expect("relationship graph node exists");
                node.kind == RelationshipNodeKind::Symbol
                    && node.symbol.as_ref().is_some_and(|symbol| {
                        role.is_none_or(|role| symbol.role == role) && symbol_matches_target(symbol, target)
                    })
            })
            .collect::<Vec<_>>();
        nodes.sort_by_key(|index| self.node(*index).id);
        nodes
    }

    fn file_nodes_for_target(&self, target: &str) -> Vec<NodeIndex> {
        let target_path = target.trim().replace('\\', "/");
        let target_path = target_path.trim_start_matches("./");
        if let Some(index) = self.file_nodes.get(target_path) {
            return vec![*index];
        }
        let paths = self
            .symbol_nodes(target, None)
            .into_iter()
            .map(|index| self.node(index).path)
            .collect::<BTreeSet<_>>();
        let mut nodes = paths.into_iter().collect::<Vec<_>>();
        if nodes.is_empty() {
            nodes = self
                .file_nodes
                .keys()
                .filter(|path| path.starts_with(&format!("{target_path}/")))
                .cloned()
                .collect();
        }
        nodes
            .into_iter()
            .filter_map(|path| self.file_nodes.get(&path).copied())
            .collect()
    }
}

fn relationship_sources(map: &MapReport) -> Vec<RelationshipSource> {
    let roots = if map.reading_evidence.project_roots.is_empty() {
        &map.project_roots
    } else {
        &map.reading_evidence.project_roots
    };
    let mut sources = if map.reading_evidence.sources.is_empty() {
        map.files
            .iter()
            .map(|file| RelationshipSource {
                path: file.path.clone(),
                language: file.language,
                status: file.status,
                symbols: file.symbols.clone(),
                limitations: file.limitations.clone(),
                project_root: crate::landmarks::project_root_for_path(&file.path, roots),
            })
            .collect::<Vec<_>>()
    } else {
        map.reading_evidence
            .sources
            .iter()
            .map(|source| RelationshipSource {
                path: source.path.clone(),
                language: source.language,
                status: source.status,
                symbols: source.symbols.clone(),
                limitations: source.limitations.clone(),
                project_root: crate::landmarks::project_root_for_path(&source.path, roots),
            })
            .collect::<Vec<_>>()
    };
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    sources
}

fn relationship_edges(map: &MapReport) -> Vec<LexicalEdge> {
    let mut edges = if map.reading_evidence.graph.is_empty() {
        map.edges.clone()
    } else {
        map.reading_evidence
            .graph
            .iter()
            .map(|edge| edge.relationship.clone())
            .collect()
    };
    edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.symbol.cmp(&right.symbol))
            .then_with(|| left.candidate_group.cmp(&right.candidate_group))
    });
    edges.dedup();
    edges
}

fn candidates_for(graph: &RelationshipGraph, request: &RelationshipRequest) -> Vec<Candidate> {
    match request.operation {
        RelationshipOperation::Symbol => graph
            .symbol_nodes(&request.target, None)
            .into_iter()
            .map(|index| {
                symbol_candidate(
                    graph,
                    index,
                    RelationshipMatchKind::Symbol,
                    "the exact symbol name matched",
                )
            })
            .collect(),
        RelationshipOperation::Definitions => graph
            .symbol_nodes(&request.target, Some(SymbolRole::Definition))
            .into_iter()
            .map(|index| {
                symbol_candidate(
                    graph,
                    index,
                    RelationshipMatchKind::Definition,
                    "the exact name matched a retained definition",
                )
            })
            .collect(),
        RelationshipOperation::References => graph
            .symbol_nodes(&request.target, Some(SymbolRole::Reference))
            .into_iter()
            .map(|index| {
                symbol_candidate(
                    graph,
                    index,
                    RelationshipMatchKind::Reference,
                    "the exact name matched a retained reference",
                )
            })
            .collect(),
        RelationshipOperation::Imports => file_relationship_candidates(
            graph,
            &request.target,
            RelationshipMatchKind::Import,
            |edge| edge.kind == RepositoryRelationshipKind::Import,
            false,
        ),
        RelationshipOperation::Dependencies => file_relationship_candidates(
            graph,
            &request.target,
            RelationshipMatchKind::Dependency,
            |_| true,
            false,
        ),
        RelationshipOperation::ReverseDependencies => file_relationship_candidates(
            graph,
            &request.target,
            RelationshipMatchKind::ReverseDependency,
            |_| true,
            true,
        ),
        RelationshipOperation::Tests => test_candidates(graph, &request.target),
        RelationshipOperation::Callers => {
            symbol_relationship_candidates(graph, &request.target, RelationshipMatchKind::Caller, true)
        }
        RelationshipOperation::Callees => {
            symbol_relationship_candidates(graph, &request.target, RelationshipMatchKind::Callee, false)
        }
    }
}

fn symbol_candidate(
    graph: &RelationshipGraph, index: NodeIndex, relation: RelationshipMatchKind, reason: &str,
) -> Candidate {
    let node = graph.node(index);
    let symbol = node.symbol.as_ref().expect("symbol operation returned a symbol node");
    let mut evidence = vec![RelationshipEvidence {
        kind: RelationshipEvidenceKind::SymbolSyntax,
        detail: format!("retained {} syntax evidence", symbol.evidence.label()),
    }];
    if node.partial {
        evidence.push(RelationshipEvidence {
            kind: RelationshipEvidenceKind::PartialSource,
            detail: "the source file was analyzed partially".to_owned(),
        });
    }
    Candidate {
        confidence: if node.partial { ConfidenceTier::Medium } else { ConfidenceTier::High },
        ambiguous: false,
        limitations: node.limitations.clone(),
        node,
        relation,
        edge: None,
        reason: reason.to_owned(),
        evidence,
    }
}

fn file_relationship_candidates(
    graph: &RelationshipGraph, target: &str, relation: RelationshipMatchKind,
    predicate: impl Fn(&RepositoryRelationship) -> bool, reverse: bool,
) -> Vec<Candidate> {
    let anchors = graph.file_nodes_for_target(target);
    let anchor_set = anchors.iter().copied().collect::<BTreeSet<_>>();
    let mut candidates = Vec::new();
    for anchor in anchors {
        let edges = if reverse {
            graph
                .graph
                .edges_directed(anchor, petgraph::Direction::Incoming)
                .collect::<Vec<_>>()
        } else {
            graph
                .graph
                .edges_directed(anchor, petgraph::Direction::Outgoing)
                .collect::<Vec<_>>()
        };
        for edge in edges {
            if !predicate(edge.weight()) {
                continue;
            }
            let other = if reverse { edge.source() } else { edge.target() };
            if anchor_set.contains(&other) {
                continue;
            }
            let relationship = edge.weight().clone();
            candidates.push(edge_candidate(
                graph,
                other,
                relation,
                relationship,
                if reverse {
                    "the target file has a retained incoming file relationship"
                } else {
                    "the target file has a retained outgoing file relationship"
                },
            ));
        }
    }
    candidates
}

fn test_candidates(graph: &RelationshipGraph, target: &str) -> Vec<Candidate> {
    let anchors = graph.file_nodes_for_target(target);
    let anchor_set = anchors.iter().copied().collect::<BTreeSet<_>>();
    let mut candidates = Vec::new();
    for anchor in anchors {
        for edge in graph
            .graph
            .edges_directed(anchor, petgraph::Direction::Incoming)
            .collect::<Vec<_>>()
        {
            let source = edge.source();
            let source_node = graph.node(source);
            if !is_test_path(&source_node.path) || anchor_set.contains(&source) {
                continue;
            }
            let mut candidate = edge_candidate(
                graph,
                source,
                RelationshipMatchKind::Test,
                edge.weight().clone(),
                "the test path has a retained lexical relationship to the target",
            );
            candidate.evidence.push(RelationshipEvidence {
                kind: RelationshipEvidenceKind::TestPath,
                detail: format!("`{}` is classified as a test path", source_node.path),
            });
            candidates.push(candidate);
        }
    }
    candidates
}

fn symbol_relationship_candidates(
    graph: &RelationshipGraph, target: &str, relation: RelationshipMatchKind, reverse: bool,
) -> Vec<Candidate> {
    let anchors = graph.symbol_nodes(target, Some(SymbolRole::Definition));
    let anchor_set = anchors.iter().copied().collect::<BTreeSet<_>>();
    let mut candidates = Vec::new();
    for anchor in anchors {
        let edges = if reverse {
            graph
                .graph
                .edges_directed(anchor, petgraph::Direction::Incoming)
                .collect::<Vec<_>>()
        } else {
            graph
                .graph
                .edges_directed(anchor, petgraph::Direction::Outgoing)
                .collect::<Vec<_>>()
        };
        for edge in edges {
            let relationship = edge.weight();
            if relationship.kind != RepositoryRelationshipKind::Call {
                continue;
            }
            let other = if reverse { edge.source() } else { edge.target() };
            if anchor_set.contains(&other) {
                continue;
            }
            candidates.push(edge_candidate(
                graph,
                other,
                relation,
                relationship.clone(),
                if reverse {
                    "the retained call edge points to the requested definition"
                } else {
                    "the retained call edge starts at the requested definition"
                },
            ));
        }
    }
    candidates
}

fn edge_candidate(
    graph: &RelationshipGraph, index: NodeIndex, relation: RelationshipMatchKind, relationship: RepositoryRelationship,
    reason: &str,
) -> Candidate {
    let node = graph.node(index);
    let mut evidence = relationship.evidence.clone();
    if node.partial {
        evidence.push(RelationshipEvidence {
            kind: RelationshipEvidenceKind::PartialSource,
            detail: "the related source file was analyzed partially".to_owned(),
        });
    }
    let mut limitations = node.limitations.clone();
    limitations.extend(relationship.limitations.clone());
    if relationship.ambiguous {
        evidence.push(RelationshipEvidence {
            kind: RelationshipEvidenceKind::Ambiguity,
            detail: "the lexical edge retained multiple definition candidates".to_owned(),
        });
        limitations.push(
            "This relationship has multiple lexical candidates and does not establish one semantic target.".to_owned(),
        );
    }
    limitations.sort();
    limitations.dedup();
    let confidence = lower_confidence_for_partial(relationship.confidence, node.partial);
    Candidate {
        node,
        relation,
        edge: Some(relationship.clone()),
        reason: reason.to_owned(),
        evidence,
        confidence,
        ambiguous: relationship.ambiguous,
        limitations,
    }
}

fn candidate_to_match(candidate: &Candidate) -> RelationshipMatch {
    let relationship_id = candidate.edge.as_ref().map(|edge| edge.id.clone());
    let id = relationship_id.as_ref().map_or_else(
        || candidate.node.id.clone(),
        |relationship_id| format!("match:{}:{}", candidate.node.id, relationship_id),
    );
    RelationshipMatch {
        id,
        node: candidate.node.clone(),
        relation: candidate.relation,
        relationship_id,
        reason: candidate.reason.clone(),
        evidence: sorted_evidence(&candidate.evidence),
        confidence: candidate.confidence,
        ambiguous: candidate.ambiguous,
        partial: candidate.node.partial,
        limitations: candidate.limitations.clone(),
    }
}

fn candidate_edges(matches: &[RelationshipMatch], candidates: &[Candidate]) -> Vec<RepositoryRelationship> {
    let selected_ids = matches
        .iter()
        .filter_map(|item| item.relationship_id.as_deref())
        .collect::<BTreeSet<_>>();
    let mut edges = candidates
        .iter()
        .filter_map(|candidate| candidate.edge.as_ref())
        .filter(|edge| selected_ids.contains(edge.id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    edges.dedup_by(|left, right| left.id == right.id);
    edges
}

fn unique_relationships(candidates: &[Candidate]) -> Vec<RepositoryRelationship> {
    let mut edges = candidates
        .iter()
        .filter_map(|candidate| candidate.edge.clone())
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    edges.dedup_by(|left, right| left.id == right.id);
    edges
}

fn estimate_payload(matches: &[RelationshipMatch], relationships: &[RepositoryRelationship]) -> usize {
    serde_json::to_string(&(matches, relationships)).map_or(usize::MAX, |value| token_count(&value))
}

fn candidate_order(left: &Candidate, right: &Candidate) -> std::cmp::Ordering {
    confidence_rank(right.confidence)
        .cmp(&confidence_rank(left.confidence))
        .then_with(|| left.ambiguous.cmp(&right.ambiguous))
        .then_with(|| left.relation.cmp(&right.relation))
        .then_with(|| left.node.path.cmp(&right.node.path))
        .then_with(|| left.node.id.cmp(&right.node.id))
        .then_with(|| {
            left.edge
                .as_ref()
                .map(|edge| edge.id.as_str())
                .cmp(&right.edge.as_ref().map(|edge| edge.id.as_str()))
        })
}

fn confidence_rank(confidence: ConfidenceTier) -> u8 {
    match confidence {
        ConfidenceTier::High => 3,
        ConfidenceTier::Medium => 2,
        ConfidenceTier::Low => 1,
    }
}

fn lower_confidence_for_partial(confidence: ConfidenceTier, partial: bool) -> ConfidenceTier {
    if partial && confidence == ConfidenceTier::High { ConfidenceTier::Medium } else { confidence }
}

fn sorted_evidence(evidence: &[RelationshipEvidence]) -> Vec<RelationshipEvidence> {
    let mut evidence = evidence.to_vec();
    evidence.sort();
    evidence.dedup();
    evidence
}

fn relationship_for_edge(
    edge: &LexicalEdge, source: &RelationshipNode, target: &RelationshipNode, kind: RepositoryRelationshipKind,
    symbol: Option<&str>,
) -> RepositoryRelationship {
    let candidate_group = if edge.candidate_group.is_empty() {
        format!("{}:{}:{}", edge.source, edge.target, edge.symbol)
    } else {
        edge.candidate_group.clone()
    };
    let id = format!(
        "relationship:{}:{}:{}:{}:{}",
        source.id,
        target.id,
        kind.label(),
        symbol.unwrap_or(&edge.symbol),
        candidate_group
    );
    let confidence = if source.partial || target.partial {
        lower_confidence_for_partial(edge.confidence, true)
    } else {
        edge.confidence
    };
    let mut limitations = Vec::new();
    if source.partial || target.partial {
        limitations.push("one endpoint was analyzed partially".to_owned());
    }
    if edge.ambiguous {
        limitations.push(
            "multiple lexical candidates were retained; this edge is not a resolved semantic relationship".to_owned(),
        );
    }
    limitations.sort();
    limitations.dedup();
    let mut evidence = vec![RelationshipEvidence {
        kind: RelationshipEvidenceKind::LexicalEdge,
        detail: format!(
            "{} lexical edge resolved by {} evidence",
            kind.label(),
            edge.resolution_reason.label()
        ),
    }];
    if source.partial || target.partial {
        evidence.push(RelationshipEvidence {
            kind: RelationshipEvidenceKind::PartialSource,
            detail: "one endpoint was analyzed partially".to_owned(),
        });
    }
    if edge.ambiguous {
        evidence.push(RelationshipEvidence {
            kind: RelationshipEvidenceKind::Ambiguity,
            detail: format!("{} lexical candidates were retained", edge.candidates.len()),
        });
    }
    RepositoryRelationship {
        id,
        source: source.id.clone(),
        target: target.id.clone(),
        source_path: source.path.clone(),
        target_path: target.path.clone(),
        kind,
        symbol: symbol.map(str::to_owned).or_else(|| Some(edge.symbol.clone())),
        ambiguous: edge.ambiguous,
        candidates: edge.candidates.clone(),
        candidate_group,
        resolution_reason: edge.resolution_reason,
        confidence,
        evidence: sorted_evidence(&evidence),
        limitations,
    }
}

fn call_owner_node(
    path: &str, reference: &SourceSymbol, symbols: &[SourceSymbol], node_ids: &BTreeMap<String, NodeIndex>,
) -> Option<NodeIndex> {
    for scope in reference.scope.iter().rev() {
        let owner = symbols.iter().find(|symbol| {
            symbol.role == SymbolRole::Definition && symbol.name == *scope && is_graph_definition(symbol)
        });
        if let Some(owner) = owner {
            return node_ids.get(&symbol_node_id(path, owner)).copied();
        }
    }
    None
}

fn import_target_names(symbol_name: &str, context: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::from([symbol_name.to_owned()]);
    let tokens = context
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    for window in tokens.windows(2) {
        if window[1] == "as" {
            names.insert(window[0].to_owned());
        }
        if window[0] == "import" {
            names.insert(window[1].to_owned());
        }
    }
    for quote in ['"', '\''] {
        if let Some(start) = context.find(quote)
            && let Some(end) = context[start + 1..].find(quote)
        {
            let module = &context[start + 1..start + 1 + end];
            if let Some(name) = module
                .trim_start_matches("./")
                .trim_start_matches("../")
                .replace('\\', "/")
                .split('/')
                .next_back()
            {
                let name = name
                    .trim_end_matches(".js")
                    .trim_end_matches(".ts")
                    .trim_end_matches(".py")
                    .trim_end_matches(".rs");
                if !name.is_empty() {
                    names.insert(name.to_owned());
                }
            }
        }
    }
    names
}

fn symbol_relationship_kind(evidence: SymbolEvidence) -> RepositoryRelationshipKind {
    match evidence {
        SymbolEvidence::Import => RepositoryRelationshipKind::Import,
        SymbolEvidence::Call => RepositoryRelationshipKind::Call,
        SymbolEvidence::TypeReference => RepositoryRelationshipKind::TypeReference,
        SymbolEvidence::MemberReference | SymbolEvidence::BareReference | SymbolEvidence::Declaration => {
            RepositoryRelationshipKind::Reference
        }
    }
}

fn is_graph_definition(symbol: &SourceSymbol) -> bool {
    symbol.role == SymbolRole::Definition
        && matches!(
            symbol.kind,
            SymbolKind::Function
                | SymbolKind::Struct
                | SymbolKind::Enum
                | SymbolKind::Trait
                | SymbolKind::Type
                | SymbolKind::Const
                | SymbolKind::Static
                | SymbolKind::Module
                | SymbolKind::Macro
                | SymbolKind::Class
                | SymbolKind::Method
                | SymbolKind::Interface
        )
}

fn symbol_matches_target(symbol: &SourceSymbol, target: &str) -> bool {
    let target = target.trim().to_ascii_lowercase();
    if target.is_empty() {
        return false;
    }
    let qualified = if symbol.scope.is_empty() {
        symbol.name.clone()
    } else {
        format!("{}::{}", symbol.scope.join("::"), symbol.name)
    };
    symbol.name.to_ascii_lowercase() == target || qualified.to_ascii_lowercase() == target
}

fn file_node_id(path: &str) -> String {
    format!("file:{path}")
}

fn symbol_node_id(path: &str, symbol: &SourceSymbol) -> String {
    format!(
        "symbol:{path}:{}:{}:{}:{}:{}:{}:{}",
        symbol.location.start.line,
        symbol.location.start.column,
        symbol.location.end.line,
        symbol.location.end.column,
        symbol.role.label(),
        symbol.kind.label(),
        symbol.name
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(name: &str, role: SymbolRole, evidence: SymbolEvidence, kind: SymbolKind) -> SourceSymbol {
        SourceSymbol {
            name: name.to_owned(),
            kind,
            role,
            scope: Vec::new(),
            location: SourceLocation { start: Position { line: 1, column: 1 }, end: Position { line: 1, column: 10 } },
            context: format!("{name}()"),
            visibility: SymbolVisibility::Public,
            evidence,
        }
    }

    fn source(path: &str, symbols: Vec<SourceSymbol>) -> RelationshipSource {
        RelationshipSource {
            path: path.to_owned(),
            language: SourceLanguage::Rust,
            status: FileAnalysisStatus::Complete,
            symbols,
            limitations: Vec::new(),
            project_root: None,
        }
    }

    #[test]
    fn petgraph_relationship_ids_are_deterministic_and_calls_are_directional() {
        let sources = vec![
            source(
                "src/lib.rs",
                vec![symbol(
                    "parse",
                    SymbolRole::Definition,
                    SymbolEvidence::Declaration,
                    SymbolKind::Function,
                )],
            ),
            source(
                "src/caller.rs",
                vec![symbol(
                    "parse",
                    SymbolRole::Reference,
                    SymbolEvidence::Call,
                    SymbolKind::Identifier,
                )],
            ),
        ];
        let edge = LexicalEdge {
            source: "src/caller.rs".to_owned(),
            target: "src/lib.rs".to_owned(),
            symbol: "parse".to_owned(),
            ambiguous: false,
            candidates: vec!["src/lib.rs".to_owned()],
            candidate_group: "group-1".to_owned(),
            resolution_reason: LexicalResolutionReason::ImportedName,
            confidence: ConfidenceTier::High,
            target_visibility: SymbolVisibility::Public,
        };
        let graph = RelationshipGraph::from_parts(sources, vec![edge]);
        let callers = symbol_relationship_candidates(&graph, "parse", RelationshipMatchKind::Caller, true);
        let callees = symbol_relationship_candidates(&graph, "parse", RelationshipMatchKind::Callee, false);
        assert_eq!(callers.len(), 1);
        assert_eq!(callees.len(), 0);
        assert!(
            callers[0]
                .edge
                .as_ref()
                .is_some_and(|edge| edge.kind == RepositoryRelationshipKind::Call)
        );
        assert!(
            callers[0]
                .edge
                .as_ref()
                .is_some_and(|edge| edge.id.starts_with("relationship:symbol:"))
        );
    }
}
