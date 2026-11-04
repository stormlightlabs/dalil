use std::borrow::Cow;

use super::*;

/// Builds the lexical edges between `files`. `project_roots` bound the
/// qualified calls of languages other than Rust: a qualifier resolves only to
/// a file under the caller's project root.
pub fn build_lexical_edges(
    files: &[SourceFile], project_roots: &[ProjectRoot], max_candidates: usize, max_edges: usize,
) -> Vec<LexicalEdge> {
    let mut definitions = BTreeMap::<(SourceLanguage, String), Vec<(String, SymbolVisibility)>>::new();
    // Methods only: the candidates for a call on a receiver of unknown type.
    let mut member_definitions = BTreeMap::<(SourceLanguage, String), Vec<(String, SymbolVisibility)>>::new();
    for file in files {
        for symbol in &file.symbols {
            if is_graph_definition(symbol) {
                definitions
                    .entry((file.language, symbol.name.clone()))
                    .or_default()
                    .push((file.path.clone(), symbol.visibility));
                if is_member_definition(symbol) {
                    member_definitions
                        .entry((file.language, symbol.name.clone()))
                        .or_default()
                        .push((file.path.clone(), symbol.visibility));
                }
            }
        }
    }
    for candidates in definitions.values_mut().chain(member_definitions.values_mut()) {
        candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.label().cmp(right.1.label())));
        candidates.dedup_by(|right, left| right.0 == left.0);
    }

    let imports = files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                file.symbols
                    .iter()
                    .filter(|symbol| symbol.role == SymbolRole::Definition && symbol.evidence == SymbolEvidence::Import)
                    .map(|symbol| (symbol.name.clone(), import_module_hints(&symbol.context, file.language)))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let modules = files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                file.symbols
                    .iter()
                    .filter(|symbol| symbol.role == SymbolRole::Definition && symbol.kind == SymbolKind::Module)
                    .map(|symbol| symbol.name.clone())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut python_names = BTreeMap::<&str, Vec<&str>>::new();
    for file in files.iter().filter(|file| file.language == SourceLanguage::Python) {
        for symbol in file.symbols.iter().filter(|symbol| {
            symbol.role == SymbolRole::Definition
                && symbol.evidence != SymbolEvidence::Import
                && symbol.scope.is_empty()
        }) {
            python_names
                .entry(symbol.name.as_str())
                .or_default()
                .push(file.path.as_str());
        }
    }
    let scope = QualifierScope::new(files, &modules, project_roots, python_names);

    let mut edges = Vec::<LexicalEdge>::new();
    // Every occurrence of a reference proposes the same edges again. Counting
    // only unseen (candidate group, target) pairs keeps `max_edges` a limit on
    // distinct edges, so repeated calls early in the file list cannot use it up.
    // Each (candidate group, target) pair maps to its edge, so a later
    // reference with another qualifier for the same targets adds the qualifier
    // to that edge rather than a new edge.
    let mut seen = BTreeMap::<(String, String), usize>::new();
    'files: for file in files {
        let bindings = import_bindings(&file.symbols);
        for symbol in &file.symbols {
            if edges.len() >= max_edges {
                break 'files;
            }
            if !is_graph_reference(symbol) {
                continue;
            }
            // A qualifier with path evidence names a module or type, not a
            // value, so its call is not one on an unknown receiver.
            let key = (file.language, symbol.name.clone());
            let qualified = definitions
                .get(&key)
                .and_then(|candidates| qualified_candidates(file, symbol, candidates, &scope));
            let unknown_receiver = qualified.is_none() && has_unknown_receiver(symbol, &bindings);
            let pool = if unknown_receiver { &member_definitions } else { &definitions };
            let all_candidates = match import_candidates(file, symbol, &definitions) {
                Some(found) => Cow::Owned(found),
                None => pool
                    .get(&key)
                    .map_or(Cow::Owned(Vec::new()), |found| Cow::Borrowed(found.as_slice())),
            };
            if all_candidates.is_empty() {
                continue;
            }
            let same_file = all_candidates
                .iter()
                .filter(|(path, _)| path == &file.path)
                .cloned()
                .collect::<Vec<_>>();
            let file_imports = imports.get(&file.path).into_iter().flatten().collect::<Vec<_>>();
            let exact_import = file_imports.iter().find(|(name, _)| name == &symbol.name);
            let (candidates, reason, confidence) = if !same_file.is_empty() {
                if symbol.evidence != SymbolEvidence::BareReference {
                    (
                        same_file,
                        LexicalResolutionReason::SameFileExplicit,
                        ConfidenceTier::High,
                    )
                } else {
                    continue;
                }
            } else if let Some((qualified_candidates, confidence)) = qualified {
                (qualified_candidates, LexicalResolutionReason::QualifiedPath, confidence)
            } else if let Some(module_candidates) = same_module_candidates(file, &all_candidates, &modules) {
                (
                    module_candidates,
                    LexicalResolutionReason::SameModule,
                    ConfidenceTier::High,
                )
            } else {
                let imported_module_candidates = all_candidates
                    .iter()
                    .filter(|(path, _)| file_imports.iter().any(|(_, hints)| module_path_matches(path, hints)))
                    .cloned()
                    .collect::<Vec<_>>();
                if !imported_module_candidates.is_empty() {
                    (
                        imported_module_candidates,
                        LexicalResolutionReason::ImportedModule,
                        ConfidenceTier::High,
                    )
                } else {
                    let Some(_) = exact_import else {
                        // A cross-file name without package or import evidence is not a dependency.
                        continue;
                    };
                    (
                        all_candidates.to_vec(),
                        LexicalResolutionReason::ImportedName,
                        ConfidenceTier::Medium,
                    )
                }
            };
            // The same evidence places a call on an unknown receiver, but any
            // method of that name may be its target, so it stays low and ambiguous.
            let (reason, confidence) = if unknown_receiver {
                (LexicalResolutionReason::MemberReceiverUnknown, ConfidenceTier::Low)
            } else {
                (reason, confidence)
            };
            let candidates = candidates.into_iter().take(max_candidates).collect::<Vec<_>>();
            if candidates.is_empty() {
                continue;
            }
            let candidate_paths = candidates.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>();
            // Qualified-path edges for the same targets are one edge, which
            // lists every qualifier that resolved to it, so `map::f()` and
            // `crate::map::f()` count once against `max_edges`.
            let qualifier = (reason == LexicalResolutionReason::QualifiedPath)
                .then(|| symbol.qualifier.clone())
                .flatten();
            let candidate_group = format!(
                "{}:{}:{}:{}",
                file.path,
                symbol.name,
                reason.label(),
                digest_hex(candidate_paths.join("\0").as_bytes())
            );
            let ambiguous = unknown_receiver || candidates.len() > 1;
            for (target, target_visibility) in &candidates {
                if let Some(&index) = seen.get(&(candidate_group.clone(), target.clone())) {
                    if let Some(qualifier) = &qualifier
                        && !edges[index].qualifiers.contains(qualifier)
                    {
                        edges[index].qualifiers.push(qualifier.clone());
                    }
                    continue;
                }
                if edges.len() >= max_edges {
                    break 'files;
                }
                seen.insert((candidate_group.clone(), target.clone()), edges.len());
                edges.push(LexicalEdge {
                    source: file.path.clone(),
                    target: target.clone(),
                    symbol: symbol.name.clone(),
                    ambiguous,
                    candidates: candidate_paths.clone(),
                    candidate_group: candidate_group.clone(),
                    resolution_reason: reason,
                    confidence,
                    target_visibility: *target_visibility,
                    qualifiers: qualifier.iter().cloned().collect(),
                });
            }
        }
    }
    for edge in &mut edges {
        edge.qualifiers.sort();
    }
    edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.symbol.cmp(&right.symbol))
            .then_with(|| left.ambiguous.cmp(&right.ambiguous))
            .then_with(|| left.candidate_group.cmp(&right.candidate_group))
    });
    edges.dedup();
    edges
}

pub fn rank_files(
    files: &[SourceFile], edges: &[LexicalEdge], project_roots: &[ProjectRoot],
    history_weights: &BTreeMap<String, u64>, settings: &MapSettings,
) -> Vec<FileRank> {
    if files.is_empty() {
        return Vec::new();
    }
    let paths = files.iter().map(|file| file.path.clone()).collect::<Vec<_>>();
    let path_set = paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut outgoing = BTreeMap::<String, Vec<&LexicalEdge>>::new();
    for edge in edges {
        outgoing.entry(edge.source.clone()).or_default().push(edge);
    }

    let seeds = settings.effective_task_seeds();
    let task_terms = seeds
        .task
        .as_deref()
        .map(lexical_task_terms)
        .unwrap_or_default()
        .into_iter()
        .map(|term| term.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let direct_matches = files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                task_seed_matches(file, project_roots, &seeds, &task_terms),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let seed_paths = direct_matches
        .iter()
        .filter_map(|(path, matches)| (!matches.is_empty()).then_some(path.clone()))
        .collect::<BTreeSet<_>>();
    let personalization = task_personalization(&paths, &seed_paths);
    let mut scores = personalization.clone();
    for _ in 0..PAGE_RANK_ITERATIONS {
        let mut next = paths
            .iter()
            .map(|path| (path.clone(), (1.0 - PAGE_RANK_DAMPING) * personalization[path]))
            .collect::<BTreeMap<_, _>>();
        let dangling = paths
            .iter()
            .filter(|path| outgoing.get(*path).is_none_or(Vec::is_empty))
            .map(|path| scores[path])
            .sum::<f64>();
        for path in &paths {
            *next.entry(path.clone()).or_default() += PAGE_RANK_DAMPING * dangling * personalization[path];
        }
        for source in &paths {
            let Some(source_edges) = outgoing.get(source) else {
                continue;
            };
            let total_weight = source_edges.iter().map(|edge| edge_weight(edge)).sum::<f64>();
            if total_weight == 0.0 {
                continue;
            }
            for edge in source_edges {
                if path_set.contains(&edge.target) {
                    let contribution = PAGE_RANK_DAMPING * scores[source] * edge_weight(edge) / total_weight;
                    *next.entry(edge.target.clone()).or_default() += contribution;
                }
            }
        }
        scores = next;
    }

    let proximity = seed_proximity(&seed_paths, edges);

    let mut ranking = files
        .iter()
        .map(|file| {
            let text_focuses = settings
                .focuses
                .iter()
                .filter(|focus| file_matches_focus(file, focus))
                .collect::<Vec<_>>();
            let path_focuses = settings
                .focus_paths
                .iter()
                .filter(|focus_path| path_matches_focus(&file.path, focus_path))
                .collect::<Vec<_>>();
            let focus_matches = text_focuses.len() + path_focuses.len();
            let mut matched_seeds = direct_matches.get(&file.path).cloned().unwrap_or_default();
            matched_seeds.extend(
                text_focuses
                    .iter()
                    .map(|focus| RankingSeedMatch { kind: RankingSeedKind::Focus, seed: (*focus).clone() }),
            );
            matched_seeds.extend(
                path_focuses
                    .iter()
                    .map(|focus| RankingSeedMatch { kind: RankingSeedKind::FocusPath, seed: (*focus).clone() }),
            );
            if let Some((_, seed_path)) = proximity.get(&file.path) {
                matched_seeds.push(RankingSeedMatch { kind: RankingSeedKind::SeedProximity, seed: seed_path.clone() });
            }
            let history = history_weights.get(&file.path).copied().unwrap_or_default();
            if history > 0 {
                matched_seeds.push(RankingSeedMatch { kind: RankingSeedKind::History, seed: file.path.clone() });
            }
            matched_seeds.sort();
            matched_seeds.dedup();

            let direct_matches = direct_matches.get(&file.path).map_or(0, Vec::len) as u64;
            let contributions = RankingContributions {
                centrality: scaled_score(scores[&file.path]),
                seed_proximity: proximity.get(&file.path).map_or(0, |(score, _)| *score),
                lexical_relevance: direct_matches.saturating_mul(300_000),
                history_evidence: history.min(20).saturating_mul(25_000),
                explicit_focus: (text_focuses.len() as u64)
                    .saturating_mul(350_000)
                    .saturating_add((path_focuses.len() as u64).saturating_mul(700_000)),
            };
            let score = contributions
                .centrality
                .saturating_add(contributions.seed_proximity)
                .saturating_add(contributions.lexical_relevance)
                .saturating_add(contributions.history_evidence)
                .saturating_add(contributions.explicit_focus);
            FileRank { path: file.path.clone(), score, focus_matches, contributions, matched_seeds }
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| right.score.cmp(&left.score).then_with(|| left.path.cmp(&right.path)));
    ranking
}

fn task_personalization(paths: &[String], seed_paths: &BTreeSet<String>) -> BTreeMap<String, f64> {
    let selected_paths = if seed_paths.is_empty() { paths.len() } else { seed_paths.len() };
    let weight = 1.0 / selected_paths as f64;
    paths
        .iter()
        .map(|path| {
            let selected = seed_paths.is_empty() || seed_paths.contains(path);
            (path.clone(), if selected { weight } else { 0.0 })
        })
        .collect()
}

fn task_seed_matches(
    file: &SourceFile, project_roots: &[ProjectRoot], seeds: &TaskSeeds, task_terms: &BTreeSet<String>,
) -> Vec<RankingSeedMatch> {
    let mut matches = Vec::new();
    for term in &seeds.search_terms {
        if file_matches_term(file, term) {
            let kind = if task_terms.contains(&term.to_ascii_lowercase()) {
                RankingSeedKind::TaskTerm
            } else {
                RankingSeedKind::SearchTerm
            };
            matches.push(RankingSeedMatch { kind, seed: term.clone() });
        }
    }
    for symbol in &seeds.symbols {
        if file
            .symbols
            .iter()
            .any(|candidate| symbol_matches_seed(candidate, symbol))
        {
            matches.push(RankingSeedMatch { kind: RankingSeedKind::Symbol, seed: symbol.clone() });
        }
    }
    for path in &seeds.paths {
        if path_matches_focus(&file.path, path) {
            matches.push(RankingSeedMatch { kind: RankingSeedKind::Path, seed: path.clone() });
        }
    }
    for language in &seeds.languages {
        if file.language == *language {
            matches.push(RankingSeedMatch { kind: RankingSeedKind::Language, seed: language.label().to_owned() });
        }
    }
    let project_root = crate::landmarks::project_root_for_path(&file.path, project_roots);
    for project in &seeds.projects {
        if project_root
            .as_deref()
            .is_some_and(|root| path_matches_focus(root, project))
        {
            matches.push(RankingSeedMatch { kind: RankingSeedKind::Project, seed: project.clone() });
        }
    }
    for change in &seeds.changes {
        match change {
            TaskChangeSeed::Path(path) if path_matches_focus(&file.path, path) => {
                matches.push(RankingSeedMatch { kind: RankingSeedKind::ChangePath, seed: path.clone() });
            }
            TaskChangeSeed::Symbol(symbol)
                if file
                    .symbols
                    .iter()
                    .any(|candidate| symbol_matches_seed(candidate, symbol)) =>
            {
                matches.push(RankingSeedMatch { kind: RankingSeedKind::ChangeSymbol, seed: symbol.clone() });
            }
            TaskChangeSeed::Path(_) | TaskChangeSeed::Symbol(_) => {}
        }
    }
    matches.sort();
    matches.dedup();
    matches
}

fn seed_proximity(seed_paths: &BTreeSet<String>, edges: &[LexicalEdge]) -> BTreeMap<String, (u64, String)> {
    use std::collections::VecDeque;

    let mut neighbors = BTreeMap::<String, BTreeSet<String>>::new();
    for edge in edges {
        neighbors
            .entry(edge.source.clone())
            .or_default()
            .insert(edge.target.clone());
        neighbors
            .entry(edge.target.clone())
            .or_default()
            .insert(edge.source.clone());
    }
    let mut distances = BTreeMap::<String, (usize, String)>::new();
    let mut pending = VecDeque::new();
    for path in seed_paths {
        distances.insert(path.clone(), (0, path.clone()));
        pending.push_back(path.clone());
    }
    while let Some(path) = pending.pop_front() {
        let (distance, seed_path) = distances[&path].clone();
        if distance >= 2 {
            continue;
        }
        for neighbor in neighbors.get(&path).into_iter().flatten() {
            if distances.contains_key(neighbor) {
                continue;
            }
            distances.insert(neighbor.clone(), (distance + 1, seed_path.clone()));
            pending.push_back(neighbor.clone());
        }
    }
    distances
        .into_iter()
        .filter_map(|(path, (distance, seed_path))| match distance {
            1 => Some((path, (250_000, seed_path))),
            2 => Some((path, (125_000, seed_path))),
            _ => None,
        })
        .collect()
}

fn file_matches_term(file: &SourceFile, term: &str) -> bool {
    let term = term.trim().to_ascii_lowercase();
    !term.is_empty()
        && (file.path.to_ascii_lowercase().contains(&term)
            || file.symbols.iter().any(|symbol| {
                symbol.name.to_ascii_lowercase().contains(&term) || symbol.context.to_ascii_lowercase().contains(&term)
            }))
}

fn symbol_matches_seed(symbol: &SourceSymbol, seed: &str) -> bool {
    let qualified = if symbol.scope.is_empty() {
        symbol.name.clone()
    } else {
        format!("{}::{}", symbol.scope.join("::"), symbol.name)
    };
    symbol.name.eq_ignore_ascii_case(seed.trim()) || qualified.eq_ignore_ascii_case(seed.trim())
}

const MAX_SELECTED_FILES: usize = 5;
const MAX_SELECTION_OMISSIONS: usize = 32;

pub fn select_snippets(
    files: &[SourceFile], edges: &[LexicalEdge], ranking: &[FileRank], project_roots: &[ProjectRoot],
    token_budget: usize, settings: &MapSettings,
) -> MapSelection {
    let seeds = settings.effective_task_seeds();
    let mut reference_counts = BTreeMap::<(String, String), u64>::new();
    let mut graph_paths = BTreeSet::new();
    for edge in edges {
        *reference_counts
            .entry((edge.target.clone(), edge.symbol.clone()))
            .or_default() += 1;
        graph_paths.insert(edge.source.as_str());
        graph_paths.insert(edge.target.as_str());
    }
    let ranks = ranking
        .iter()
        .map(|rank| (rank.path.as_str(), rank))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = Vec::new();
    for file in files {
        let rank = ranks.get(file.path.as_str()).copied();
        let file_score = rank.map_or(0, |rank| rank.score);
        let task_relevant = rank.is_some_and(file_has_direct_task_evidence);
        let role = snippet_file_role(&file.path, project_roots);
        let generated = !file.classifications.is_empty();
        let strong_file = task_relevant
            || rank.is_some_and(|rank| rank.contributions.history_evidence > 0)
            || graph_paths.contains(file.path.as_str())
            || role != SnippetFileRole::Source
            || file
                .symbols
                .iter()
                .any(|symbol| symbol.visibility == SymbolVisibility::Public);
        if !strong_file || (generated && !task_relevant) {
            continue;
        }
        let project_root = crate::landmarks::project_root_for_path(&file.path, project_roots);
        let subsystem = snippet_subsystem(&file.path, project_root.as_deref());
        for symbol in file
            .symbols
            .iter()
            .filter(|symbol| symbol.role == SymbolRole::Definition)
        {
            let reference_count = reference_counts
                .get(&(file.path.clone(), symbol.name.clone()))
                .copied()
                .unwrap_or_default();
            let focus_boost = settings
                .focuses
                .iter()
                .filter(|focus| symbol_matches_focus(symbol, focus))
                .count() as u64
                * 250_000;
            let task_symbol_boost = seeds
                .symbols
                .iter()
                .chain(seeds.changes.iter().filter_map(|change| match change {
                    TaskChangeSeed::Symbol(symbol) => Some(symbol),
                    TaskChangeSeed::Path(_) => None,
                }))
                .filter(|seed| symbol_matches_seed(symbol, seed))
                .count() as u64
                * 300_000;
            let symbol_score = file_score
                .saturating_add(reference_count.saturating_mul(1_000))
                .saturating_add(focus_boost)
                .saturating_add(task_symbol_boost);
            candidates.push(SnippetCandidate {
                path: file.path.clone(),
                language: file.language,
                symbol: symbol.clone(),
                score: symbol_score,
                task_relevant,
                partial: file.status == FileAnalysisStatus::Partial,
                generated,
                project_root: project_root.clone(),
                subsystem: subsystem.clone(),
                role,
            });
        }
    }
    candidates.sort_by(snippet_candidate_order);

    // Keep the strongest declaration for each file before applying diversity. A
    // file set is more useful for orientation than five declarations from one hub.
    let mut candidates_by_path = BTreeMap::new();
    for candidate in candidates {
        candidates_by_path.entry(candidate.path.clone()).or_insert(candidate);
    }
    let mut remaining = candidates_by_path.into_values().collect::<Vec<_>>();
    let candidate_count = remaining.len();
    let primary_languages = primary_languages(files);
    let mut selected_candidates = Vec::new();
    let mut snippets = Vec::new();
    let mut estimated_tokens = 0;

    while snippets.len() < MAX_SELECTED_FILES {
        let remaining_tokens = token_budget.saturating_sub(estimated_tokens);
        let Some((index, symbol, cost, truncated)) =
            best_snippet_candidate(&remaining, &selected_candidates, remaining_tokens)
        else {
            break;
        };
        let candidate = remaining.remove(index);
        estimated_tokens = estimated_tokens.saturating_add(cost);
        snippets.push(MapSnippet {
            path: candidate.path.clone(),
            language: candidate.language,
            symbol,
            score: candidate.score,
            estimated_tokens: cost,
            truncated,
        });
        selected_candidates.push(candidate);
    }

    let omitted_relevant_paths = remaining
        .iter()
        .filter(|candidate| candidate.task_relevant)
        .take(MAX_SELECTION_OMISSIONS)
        .map(|candidate| MapSelectionOmission {
            path: candidate.path.clone(),
            reason: if fit_snippet(candidate, token_budget).is_none() {
                format!("the {token_budget}-token map budget cannot fit this declaration")
            } else if snippets.len() == MAX_SELECTED_FILES {
                "the five-file selection bound retained a more diverse or stronger task-relevant path".to_owned()
            } else {
                "the remaining map token budget could not fit this declaration".to_owned()
            },
        })
        .collect::<Vec<_>>();
    let shortfall = (snippets.len() < MIN_SELECTED_FILES).then(|| MapSelectionShortfall {
        target_minimum: MIN_SELECTED_FILES,
        returned: snippets.len(),
        reason: if candidate_count < MIN_SELECTED_FILES {
            format!("only {candidate_count} source files had strong structural or task evidence")
        } else {
            format!(
                "the {}-token map budget fit only {} strong source file(s)",
                token_budget,
                snippets.len()
            )
        },
    });

    MapSelection { token_budget, estimated_tokens, snippets, primary_languages, omitted_relevant_paths, shortfall }
}

fn file_has_direct_task_evidence(rank: &FileRank) -> bool {
    rank.matched_seeds.iter().any(|seed| {
        matches!(
            seed.kind,
            RankingSeedKind::TaskTerm
                | RankingSeedKind::SearchTerm
                | RankingSeedKind::Symbol
                | RankingSeedKind::Path
                | RankingSeedKind::Language
                | RankingSeedKind::Project
                | RankingSeedKind::ChangePath
                | RankingSeedKind::ChangeSymbol
                | RankingSeedKind::Focus
                | RankingSeedKind::FocusPath
        )
    })
}

fn primary_languages(files: &[SourceFile]) -> Vec<SourceLanguage> {
    let mut counts = BTreeMap::<SourceLanguage, usize>::new();
    for file in files {
        if file.classifications.is_empty() {
            *counts.entry(file.language).or_default() += 1;
        }
    }
    let maximum = counts.values().copied().max().unwrap_or_default();
    let mut languages = counts
        .into_iter()
        .filter(|(_, count)| *count * 3 >= maximum)
        .collect::<Vec<_>>();
    languages.sort_by(|(left_language, left_count), (right_language, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_language.cmp(right_language))
    });
    languages.into_iter().take(3).map(|(language, _)| language).collect()
}

fn best_snippet_candidate(
    candidates: &[SnippetCandidate], selected: &[SnippetCandidate], token_budget: usize,
) -> Option<(usize, SourceSymbol, usize, bool)> {
    let diversity = SnippetDiversity::new(candidates, selected);
    let mut best = None;
    for (index, candidate) in candidates.iter().enumerate() {
        let Some((symbol, cost, truncated)) = fit_snippet(candidate, token_budget) else {
            continue;
        };
        let score = diversified_snippet_score(candidate, &diversity);
        let replace = best.as_ref().is_none_or(|(best_index, _, best_cost, _)| {
            let best_candidate = &candidates[*best_index];
            let best_score = diversified_snippet_score(best_candidate, &diversity);
            score > best_score
                || (score == best_score && cost < *best_cost)
                || (score == best_score
                    && cost == *best_cost
                    && snippet_candidate_order(candidate, best_candidate).is_lt())
        });
        if replace {
            best = Some((index, symbol, cost, truncated));
        }
    }
    best
}

struct SnippetDiversity {
    selected_languages: BTreeSet<SourceLanguage>,
    selected_subsystems: BTreeSet<String>,
    selected_roots: BTreeSet<String>,
    has_unrepresented_subsystem: bool,
    has_unrepresented_language: bool,
    has_unrepresented_root: bool,
}

impl SnippetDiversity {
    fn new(candidates: &[SnippetCandidate], selected: &[SnippetCandidate]) -> Self {
        let selected_languages = selected
            .iter()
            .map(|candidate| candidate.language)
            .collect::<BTreeSet<_>>();
        let selected_subsystems = selected
            .iter()
            .map(|candidate| candidate.subsystem.clone())
            .collect::<BTreeSet<_>>();
        let selected_roots = selected
            .iter()
            .filter_map(|candidate| candidate.project_root.clone())
            .collect::<BTreeSet<_>>();
        Self {
            has_unrepresented_subsystem: candidates
                .iter()
                .any(|candidate| !selected_subsystems.contains(candidate.subsystem.as_str())),
            has_unrepresented_language: candidates
                .iter()
                .any(|candidate| !selected_languages.contains(&candidate.language)),
            has_unrepresented_root: candidates.iter().any(|candidate| {
                candidate
                    .project_root
                    .as_deref()
                    .is_some_and(|root| !selected_roots.contains(root))
            }),
            selected_languages,
            selected_subsystems,
            selected_roots,
        }
    }
}

fn diversified_snippet_score(candidate: &SnippetCandidate, diversity: &SnippetDiversity) -> u64 {
    if candidate.task_relevant {
        return candidate.score;
    }

    let mut score = candidate.score;
    if candidate.partial {
        score /= 2;
    }
    if candidate.generated {
        score /= 4;
    }
    if diversity.has_unrepresented_subsystem && diversity.selected_subsystems.contains(candidate.subsystem.as_str()) {
        score /= 3;
    }
    if diversity.has_unrepresented_language && diversity.selected_languages.contains(&candidate.language) {
        score = score.saturating_mul(2) / 3;
    }
    if diversity.has_unrepresented_root
        && candidate
            .project_root
            .as_deref()
            .is_some_and(|root| diversity.selected_roots.contains(root))
    {
        score = score.saturating_mul(3) / 4;
    }
    score.saturating_add(match candidate.role {
        SnippetFileRole::EntryPoint => 1_000_000,
        SnippetFileRole::Gateway => 750_000,
        SnippetFileRole::Test => 500_000,
        SnippetFileRole::Source => 0,
    })
}

fn snippet_candidate_order(left: &SnippetCandidate, right: &SnippetCandidate) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| left.path.cmp(&right.path))
        .then_with(|| location_key(Some(&left.symbol.location)).cmp(&location_key(Some(&right.symbol.location))))
        .then_with(|| left.symbol.name.cmp(&right.symbol.name))
}

fn snippet_file_role(path: &str, project_roots: &[ProjectRoot]) -> SnippetFileRole {
    let name = path.rsplit('/').next().unwrap_or(path);
    let lowercase_name = name.to_ascii_lowercase();
    let declared_entry_point = project_roots
        .iter()
        .flat_map(|root| &root.manifest_metadata)
        .any(|metadata| {
            metadata
                .runtime_entry_points
                .iter()
                .chain(&metadata.library_exports)
                .any(|target| target.resolved_path.as_deref() == Some(path))
        });
    if declared_entry_point
        || path.starts_with("examples/")
        || path.contains("/examples/")
        || matches!(
            name,
            "main.rs"
                | "main.py"
                | "main.rb"
                | "main.ts"
                | "main.js"
                | "index.ts"
                | "index.js"
                | "index.tsx"
                | "index.jsx"
        )
    {
        SnippetFileRole::EntryPoint
    } else if path.starts_with("tests/") || path.contains("/tests/") || name.ends_with("_test.go") {
        SnippetFileRole::Test
    } else if ["api", "gateway", "handler", "router", "server"]
        .iter()
        .any(|term| lowercase_name.contains(term))
    {
        SnippetFileRole::Gateway
    } else {
        SnippetFileRole::Source
    }
}

fn snippet_subsystem(path: &str, project_root: Option<&str>) -> String {
    let relative = project_root
        .filter(|root| *root != ".")
        .and_then(|root| path.strip_prefix(root).and_then(|path| path.strip_prefix('/')))
        .unwrap_or(path);
    relative
        .rsplit_once('/')
        .map_or(".".to_owned(), |(parent, _)| parent.to_owned())
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

/// Whether a definition can be the target of a call on a receiver of unknown
/// type: a method, or a function defined inside a type's body.
pub(crate) fn is_member_definition(symbol: &SourceSymbol) -> bool {
    symbol.role == SymbolRole::Definition && (symbol.kind == SymbolKind::Method || symbol.member)
}

/// The names a file's imports bind: the import's name, the last segment of
/// its path, such as `fmt` for Go's `import "fmt"`, and a variable declared on
/// the import's line, such as `helper` in Lua's
/// `local helper = require('src.lua_helper')` or Zig's `const helper = @import(…)`.
pub(crate) fn import_bindings(symbols: &[SourceSymbol]) -> BTreeSet<&str> {
    let imports = symbols
        .iter()
        .filter(|symbol| symbol.role == SymbolRole::Definition && symbol.evidence == SymbolEvidence::Import)
        .collect::<Vec<_>>();
    let import_lines = imports
        .iter()
        .map(|symbol| symbol.location.start.line)
        .collect::<BTreeSet<_>>();
    let variables = symbols.iter().filter(|symbol| {
        symbol.role == SymbolRole::Definition
            && symbol.kind == SymbolKind::Variable
            && import_lines.contains(&symbol.location.start.line)
    });
    imports
        .iter()
        .flat_map(|symbol| {
            let last = symbol.name.rsplit(['/', '.', ':']).next().unwrap_or(&symbol.name);
            [symbol.name.as_str(), last]
        })
        .chain(variables.map(|symbol| symbol.name.as_str()))
        .collect()
}

/// Whether a reference is a member call on a receiver whose type is not known:
/// not `self`, `Self`, `this`, `super`, `base` or `cls`, and not a name an
/// import binds, which makes `fmt.Println()` a path rather than a value.
pub(crate) fn has_unknown_receiver(symbol: &SourceSymbol, import_bindings: &BTreeSet<&str>) -> bool {
    symbol.receiver.as_deref().is_some_and(|receiver| {
        !matches!(receiver, "self" | "Self" | "this" | "super" | "base" | "cls") && !import_bindings.contains(receiver)
    })
}

fn is_graph_reference(symbol: &SourceSymbol) -> bool {
    let import_definition = symbol.role == SymbolRole::Definition && symbol.evidence == SymbolEvidence::Import;
    let explicit_reference = symbol.role == SymbolRole::Reference
        && !matches!(
            symbol.evidence,
            SymbolEvidence::BareReference | SymbolEvidence::MemberReference
        )
        && symbol.kind != SymbolKind::Field;
    (import_definition || explicit_reference) && !is_generic_name(&symbol.name)
}

fn import_candidates(
    file: &SourceFile, symbol: &SourceSymbol,
    definitions: &BTreeMap<(SourceLanguage, String), Vec<(String, SymbolVisibility)>>,
) -> Option<Vec<(String, SymbolVisibility)>> {
    if symbol.role != SymbolRole::Definition || symbol.evidence != SymbolEvidence::Import {
        return None;
    }
    let hints = import_module_hints(&symbol.context, file.language);
    let names = import_symbol_names(&symbol.name, &symbol.context, &hints);
    let mut candidates = definitions
        .iter()
        .filter(|((language, name), _)| *language == file.language && names.contains(name))
        .flat_map(|(_, candidates)| candidates.iter().cloned())
        .filter(|(path, _)| hints.is_empty() || module_path_matches(path, &hints))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.label().cmp(right.1.label())));
    candidates.dedup_by(|right, left| right.0 == left.0);
    (!candidates.is_empty()).then_some(candidates)
}

fn import_symbol_names(symbol_name: &str, context: &str, hints: &[String]) -> BTreeSet<String> {
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
    names.extend(
        hints
            .iter()
            .filter_map(|hint| hint.rsplit('/').next())
            .map(str::to_owned),
    );
    names
}

fn import_module_hints(context: &str, language: SourceLanguage) -> Vec<String> {
    let mut hints = Vec::new();
    let mut quoted = None;
    for quote in ['"', '\''] {
        if let Some(start) = context.find(quote)
            && let Some(end) = context[start + 1..].find(quote)
        {
            quoted = Some(context[start + 1..start + 1 + end].to_owned());
            break;
        }
    }
    if let Some(value) = quoted {
        let normalized = normalize_module_hint(&value);
        hints.push(normalized.clone());
        if language == SourceLanguage::Lua && normalized.contains('.') {
            hints.push(normalized.replace('.', "/"));
        }
    }
    let words = context.split_whitespace().collect::<Vec<_>>();
    if let Some(index) = words.iter().position(|word| *word == "from")
        && let Some(module) = words.get(index + 1)
    {
        hints.push(normalize_module_hint(module));
    }
    hints.extend(
        context
            .split(|character: char| character.is_whitespace() || matches!(character, ';' | ',' | '(' | ')'))
            .filter(|part| part.contains("::") || part.contains('/'))
            .map(normalize_module_hint),
    );
    hints.retain(|hint| !hint.is_empty());
    hints.sort();
    hints.dedup();
    hints
}

/// What qualified calls resolve against, built once per graph so that each
/// reference does lookups rather than scans: every file by path, the module
/// names each file declares, each file's project root, and indexes of the
/// files that may be a qualifier's target.
struct QualifierScope<'a> {
    files: BTreeMap<&'a str, &'a SourceFile>,
    modules: &'a BTreeMap<String, BTreeSet<String>>,
    /// Each file's project root.
    roots: BTreeMap<&'a str, Option<String>>,
    /// The Python files that define each name at module level, other than
    /// by importing it.
    python_names: BTreeMap<&'a str, Vec<&'a str>>,
    /// The files whose module path, as [`module_path`] gives it, ends in each
    /// last segment, such as `bar` for `lib/foo/bar.rb`. Ruby and Python
    /// qualifiers look up their last segment here.
    module_files: BTreeMap<(SourceLanguage, &'a str), Vec<&'a str>>,
    /// The files that declare each type, for Java and C#.
    type_files: BTreeMap<(SourceLanguage, &'a str), Vec<&'a str>>,
    /// The Lua files that assign each global table.
    lua_tables: BTreeMap<&'a str, Vec<&'a str>>,
    /// The module names declared in each Rust crate, keyed by the source root
    /// [`crate_source_root`] gives.
    crate_modules: BTreeMap<&'a str, BTreeSet<&'a str>>,
}

impl<'a> QualifierScope<'a> {
    fn new(
        files: &'a [SourceFile], modules: &'a BTreeMap<String, BTreeSet<String>>, project_roots: &'a [ProjectRoot],
        python_names: BTreeMap<&'a str, Vec<&'a str>>,
    ) -> Self {
        let mut module_files = BTreeMap::<_, Vec<_>>::new();
        let mut type_files = BTreeMap::<_, Vec<_>>::new();
        let mut lua_tables = BTreeMap::<_, Vec<_>>::new();
        for file in files {
            let path = file.path.as_str();
            match file.language {
                SourceLanguage::Ruby | SourceLanguage::Python => {
                    let last = module_path(path).rsplit('/').next().unwrap_or(path);
                    module_files.entry((file.language, last)).or_default().push(path);
                }
                SourceLanguage::Java | SourceLanguage::CSharp => {
                    let declared = file
                        .symbols
                        .iter()
                        .filter(|symbol| is_type_declaration(symbol))
                        .map(|symbol| symbol.name.as_str())
                        .collect::<BTreeSet<_>>();
                    for name in declared {
                        type_files.entry((file.language, name)).or_default().push(path);
                    }
                }
                SourceLanguage::Lua => {
                    let tables = file
                        .symbols
                        .iter()
                        .filter(|symbol| is_global_table(symbol))
                        .map(|symbol| symbol.name.as_str())
                        .collect::<BTreeSet<_>>();
                    for table in tables {
                        lua_tables.entry(table).or_default().push(path);
                    }
                }
                _ => {}
            }
        }
        let mut crate_modules = BTreeMap::<&str, BTreeSet<&str>>::new();
        for file in files.iter().filter(|file| file.language == SourceLanguage::Rust) {
            let root = crate_source_root(&file.path);
            if crate_modules.contains_key(root) {
                continue;
            }
            let declared = modules
                .iter()
                .filter(|(path, _)| same_crate(&file.path, path))
                .flat_map(|(_, names)| names.iter().map(String::as_str))
                .collect();
            crate_modules.insert(root, declared);
        }
        Self {
            files: files.iter().map(|file| (file.path.as_str(), file)).collect(),
            modules,
            roots: files
                .iter()
                .map(|file| {
                    (
                        file.path.as_str(),
                        crate::landmarks::project_root_for_path(&file.path, project_roots),
                    )
                })
                .collect(),
            python_names,
            module_files,
            type_files,
            lua_tables,
            crate_modules,
        }
    }

    fn same_project(&self, source: &str, candidate: &str) -> bool {
        self.roots.get(source) == self.roots.get(candidate)
    }

    /// The candidates among `targets` in the caller's project root, other
    /// than the caller, whose file passes `keep`, in path order. `candidates`
    /// is sorted by path, so each target is a binary search.
    fn pick<'t>(
        &self, file: &SourceFile, candidates: &[(String, SymbolVisibility)],
        targets: impl IntoIterator<Item = &'t str>, keep: impl Fn(&SourceFile) -> bool,
    ) -> Option<Vec<(String, SymbolVisibility)>> {
        let mut matching = targets
            .into_iter()
            .filter(|path| *path != file.path && self.same_project(&file.path, path))
            .filter_map(|path| {
                let index = candidates
                    .binary_search_by(|(candidate, _)| candidate.as_str().cmp(path))
                    .ok()?;
                self.files
                    .get(path)
                    .is_some_and(|target| keep(target))
                    .then(|| candidates[index].clone())
            })
            .collect::<Vec<_>>();
        matching.sort_by(|left, right| left.0.cmp(&right.0));
        matching.dedup_by(|right, left| right.0 == left.0);
        (!matching.is_empty()).then_some(matching)
    }
}

/// The candidates a qualified call resolves to, and the confidence its
/// evidence gives, per language:
///
/// - Rust: the module path, as [`qualified_path_candidates`] reads it.
/// - Ruby: the constant path in snake case, so `Foo::Bar.baz` and
///   `Foo::Bar::baz` resolve to a file ending in `foo/bar.rb`.
/// - Python: the qualifier's first name must be bound by an import, which
///   gives the module path: `pkg.mod.f()` after `import pkg.mod`, or `mod.f()`
///   after `from pkg import mod`, resolves to `pkg/mod.py` or
///   `pkg/mod/__init__.py`, with case kept. When `pkg` itself defines `mod`,
///   such as a class, the call resolves to that file instead.
/// - Java and C#: the file that declares the qualifier's type in the
///   qualifier's package or namespace, read from its `package` or `namespace`
///   declaration rather than its directory. A lowercase name such as `x` in
///   `x.get()` is a variable, not a type, and is not resolved.
/// - Lua: a file that assigns the global table `M = {…}` and defines the
///   called function on it, unless the caller binds `M` with `local`. The
///   qualifier names no path, so this is medium.
///
/// Every language but Rust stays inside the caller's project root. When an
/// import binds the qualifier's first name, such as Lua's
/// `local M = require("a")`, the files that import names take precedence: a
/// result outside them is dropped, and the call resolves through the import.
/// Rust and Python resolve the qualifier through that import already.
fn qualified_candidates(
    file: &SourceFile, symbol: &SourceSymbol, candidates: &[(String, SymbolVisibility)], scope: &QualifierScope<'_>,
) -> Option<(Vec<(String, SymbolVisibility)>, ConfidenceTier)> {
    let found = qualified_candidates_by_language(file, symbol, candidates, scope)?;
    // Rust and Python read the qualifier through the import that binds it
    // already, so their results are that import's evidence.
    if matches!(file.language, SourceLanguage::Rust | SourceLanguage::Python) {
        return Some(found);
    }
    let hints = qualifier_import_hints(file, symbol.qualifier.as_deref()?);
    if hints.is_empty() {
        return Some(found);
    }
    let imported = candidates
        .iter()
        .filter(|(path, _)| module_path_matches(path, &hints))
        .map(|(path, _)| path.as_str())
        .collect::<BTreeSet<_>>();
    let refines_import = imported.is_empty() || found.0.iter().all(|(path, _)| imported.contains(path.as_str()));
    refines_import.then_some(found)
}

fn qualified_candidates_by_language(
    file: &SourceFile, symbol: &SourceSymbol, candidates: &[(String, SymbolVisibility)], scope: &QualifierScope<'_>,
) -> Option<(Vec<(String, SymbolVisibility)>, ConfidenceTier)> {
    let qualifier = symbol.qualifier.as_deref()?;
    match file.language {
        SourceLanguage::Rust => {
            qualified_path_candidates(file, symbol, candidates, scope).map(|found| (found, ConfidenceTier::High))
        }
        SourceLanguage::Ruby => {
            let module = qualifier
                .trim_start_matches("::")
                .split("::")
                .map(snake_case)
                .collect::<Vec<_>>()
                .join("/");
            let last = module.rsplit('/').next().unwrap_or(&module);
            let targets = scope.module_files.get(&(file.language, last)).into_iter().flatten();
            scope
                .pick(file, candidates, targets.copied(), |target| {
                    module_path_ends_with(&target.path, &module)
                })
                .map(|found| (found, ConfidenceTier::High))
        }
        SourceLanguage::Python => {
            let qualifier = python_qualifier(file, qualifier)?;
            // `from pkg import X` binds a name `pkg` defines, such as a class,
            // before it binds the submodule `pkg/X.py`.
            if let Some((parent, name)) = &qualifier.member_of {
                let definers = scope
                    .python_names
                    .get(name.as_str())
                    .into_iter()
                    .flatten()
                    .filter(|path| parent.matches(path))
                    .copied()
                    .collect::<Vec<_>>();
                if definers.iter().any(|path| scope.same_project(&file.path, path)) {
                    return scope
                        .pick(file, candidates, definers, |_| true)
                        .map(|found| (found, ConfidenceTier::High));
                }
            }
            let targets = scope
                .module_files
                .get(&(file.language, qualifier.module.last_segment()))
                .into_iter()
                .flatten();
            scope
                .pick(file, candidates, targets.copied(), |target| {
                    qualifier.module.matches(&target.path)
                })
                .map(|found| (found, ConfidenceTier::High))
        }
        SourceLanguage::Java | SourceLanguage::CSharp => {
            let (type_name, packages) = declared_type_names(file, qualifier, scope.modules)?;
            let targets = scope
                .type_files
                .get(&(file.language, type_name.as_str()))
                .into_iter()
                .flatten();
            scope
                .pick(file, candidates, targets.copied(), |target| {
                    let target_packages = scope.modules.get(&target.path);
                    packages.iter().any(|package| {
                        if package.is_empty() {
                            target_packages.is_none_or(BTreeSet::is_empty)
                        } else {
                            target_packages.is_some_and(|declared| declared.contains(package))
                        }
                    })
                })
                .map(|found| (found, ConfidenceTier::High))
        }
        SourceLanguage::Lua => {
            if binds_local(file, qualifier) {
                return None;
            }
            let targets = scope.lua_tables.get(qualifier).into_iter().flatten();
            scope
                .pick(file, candidates, targets.copied(), |target| {
                    defines_table_function(target, qualifier, &symbol.name)
                })
                .map(|found| (found, ConfidenceTier::Medium))
        }
        _ => None,
    }
}

/// The module hints of the imports that bind a qualifier's first name, as
/// [`import_bindings`] reads a binding: the import's name, the last segment
/// of its path, or a variable declared on its line.
fn qualifier_import_hints(file: &SourceFile, qualifier: &str) -> Vec<String> {
    let qualifier = qualifier.trim_start_matches("::");
    let qualifier = qualifier.strip_prefix("global::").unwrap_or(qualifier);
    let Some(head) = qualifier.split(['.', ':']).find(|segment| !segment.is_empty()) else {
        return Vec::new();
    };
    let variable_lines = file
        .symbols
        .iter()
        .filter(|symbol| {
            symbol.role == SymbolRole::Definition && symbol.kind == SymbolKind::Variable && symbol.name == head
        })
        .map(|symbol| symbol.location.start.line)
        .collect::<BTreeSet<_>>();
    let mut hints = file
        .symbols
        .iter()
        .filter(|import| {
            import.role == SymbolRole::Definition
                && import.evidence == SymbolEvidence::Import
                && (import.name == head
                    || import.name.rsplit(['/', '.', ':']).next() == Some(head)
                    || variable_lines.contains(&import.location.start.line))
        })
        .flat_map(|import| import_module_hints(&import.context, file.language))
        .collect::<Vec<_>>();
    hints.sort();
    hints.dedup();
    hints
}

/// Whether `path`, as [`module_path`] gives it, is `module` or ends with `/module`.
fn module_path_ends_with(path: &str, module: &str) -> bool {
    let normalized = module_path(path);
    !module.is_empty() && (normalized == module || normalized.ends_with(&format!("/{module}")))
}

/// A file's path without its extension, with a Python package's `/__init__`
/// dropped: `pkg/mod/__init__.py` is `pkg/mod`. Case is kept, since Python
/// imports and Ruby's file names for constants are case-sensitive.
fn module_path(path: &str) -> &str {
    let stem = match path.rfind('.') {
        Some(dot) if !path[dot..].contains('/') => &path[..dot],
        _ => path,
    };
    stem.strip_suffix("/__init__").unwrap_or(stem)
}

/// Ruby's file name for a constant: `HTTPClient` is `http_client`.
fn snake_case(constant: &str) -> String {
    let characters = constant.chars().collect::<Vec<_>>();
    let mut output = String::new();
    for (index, character) in characters.iter().enumerate() {
        if character.is_uppercase() && index > 0 {
            let previous = characters[index - 1];
            let next_lowercase = characters.get(index + 1).is_some_and(|next| next.is_lowercase());
            if previous.is_lowercase() || previous.is_ascii_digit() || (previous.is_uppercase() && next_lowercase) {
                output.push('_');
            }
        }
        output.extend(character.to_lowercase());
    }
    output
}

enum PythonModule {
    /// A module path matched at the end of a file's path, such as `pkg/mod`.
    Absolute(String),
    /// A repository path from a relative import, such as `app/pkg/mod`.
    Relative(String),
}

impl PythonModule {
    /// The module a dotted import path names from `file`: `pkg.mod` is
    /// `pkg/mod` anywhere, and `..pkg` is `pkg` in the caller's parent package.
    fn from_import(file: &SourceFile, dotted: &str, rest: &[&str]) -> Self {
        let dots = dotted.len() - dotted.trim_start_matches('.').len();
        let mut module = dotted
            .trim_start_matches('.')
            .split('.')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        module.extend(rest);
        if dots == 0 {
            return Self::Absolute(module.join("/"));
        }
        let mut directory = repository_parent(&file.path);
        for _ in 1..dots {
            directory = repository_parent(directory);
        }
        Self::Relative(
            std::iter::once(directory)
                .chain(module)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("/"),
        )
    }

    /// The module's last segment, such as `mod` for `pkg/mod`.
    fn last_segment(&self) -> &str {
        let (Self::Absolute(module) | Self::Relative(module)) = self;
        module.rsplit('/').next().unwrap_or(module)
    }

    fn matches(&self, path: &str) -> bool {
        match self {
            Self::Absolute(module) => module_path_ends_with(path, module),
            Self::Relative(module) => module_path(path) == module,
        }
    }
}

/// What a Python qualifier names.
struct PythonQualifier {
    /// The qualifier read as a module.
    module: PythonModule,
    /// For a qualifier `X` bound by `from pkg import X`, the module `pkg` and
    /// the name `X`, which may be something `pkg` defines rather than a
    /// submodule.
    member_of: Option<(PythonModule, String)>,
}

/// The module a Python qualifier names, from the import that binds its first
/// name: `import pkg.mod` binds `pkg`, so `pkg.mod` is `pkg/mod`;
/// `import pkg.mod as m` makes `m.sub` `pkg/mod/sub`; `from pkg import mod`
/// makes `mod` `pkg/mod`; and `from . import mod` is `mod` beside the caller.
fn python_qualifier(file: &SourceFile, qualifier: &str) -> Option<PythonQualifier> {
    let segments = qualifier.split('.').collect::<Vec<_>>();
    let (head, rest) = segments.split_first()?;
    for import in file
        .symbols
        .iter()
        .filter(|import| import.role == SymbolRole::Definition && import.evidence == SymbolEvidence::Import)
    {
        let words = import
            .context
            .split(|character: char| character.is_whitespace() || matches!(character, ',' | '(' | ')'))
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        let Some((dotted, imported)) = python_import_binding(&words, head) else {
            continue;
        };
        let member_of = imported.filter(|_| rest.is_empty()).map(|name| {
            let parent = dotted.strip_suffix(name.as_str()).unwrap_or(&dotted);
            let parent = if parent.chars().all(|character| character == '.') {
                parent
            } else {
                parent.strip_suffix('.').unwrap_or(parent)
            };
            (PythonModule::from_import(file, parent, &[]), name)
        });
        return Some(PythonQualifier { module: PythonModule::from_import(file, &dotted, rest), member_of });
    }
    None
}

/// The dotted module that `name` stands for in one Python import statement,
/// split into words, or `None` when the statement does not bind `name`. For
/// `from … import X` it also gives `X`, the name imported.
fn python_import_binding(words: &[&str], name: &str) -> Option<(String, Option<String>)> {
    let alias_of = |index: usize| {
        (words.get(index + 1) == Some(&"as"))
            .then(|| words.get(index + 2).copied())
            .flatten()
    };
    match *words.first()? {
        "import" => words.iter().enumerate().skip(1).find_map(|(index, word)| {
            if *word == "as" || words.get(index.wrapping_sub(1)) == Some(&"as") {
                return None;
            }
            match alias_of(index) {
                Some(alias) => (alias == name).then(|| ((*word).to_owned(), None)),
                // `import pkg.mod` binds only `pkg`.
                None => (word.split('.').next() == Some(name)).then(|| (name.to_owned(), None)),
            }
        }),
        "from" => {
            let from = words.get(1)?;
            let names = words.iter().position(|word| *word == "import")? + 1;
            words.iter().enumerate().skip(names).find_map(|(index, word)| {
                if *word == "as" || words.get(index - 1) == Some(&"as") {
                    return None;
                }
                let bound = alias_of(index).unwrap_or(word);
                (bound == name).then(|| {
                    let dotted = if from.chars().all(|character| character == '.') {
                        format!("{from}{word}")
                    } else {
                        format!("{from}.{word}")
                    };
                    (dotted, Some((*word).to_owned()))
                })
            })
        }
        _ => None,
    }
}

/// The type a Java or C# qualifier names and the packages or namespaces it
/// may be declared in. `a.b.C` is `C` in `a.b`. A bare `C` is in the caller's
/// own package or namespace, the package of a Java `import a.b.C;` or
/// `import a.b.*;`, or a C# `using` namespace; an empty package is the
/// default one. C#'s `global::` prefix is dropped.
fn declared_type_names(
    file: &SourceFile, qualifier: &str, modules: &BTreeMap<String, BTreeSet<String>>,
) -> Option<(String, Vec<String>)> {
    let qualifier = qualifier.strip_prefix("global::").unwrap_or(qualifier);
    let (package, type_name) = match qualifier.rsplit_once('.') {
        Some((package, type_name)) => (Some(package), type_name),
        None => (None, qualifier),
    };
    if type_name.is_empty() || qualifier.contains("::") {
        return None;
    }
    if let Some(package) = package {
        return Some((type_name.to_owned(), vec![package.to_owned()]));
    }
    // A lowercase name such as `x` in `x.get()` is a variable, not a type.
    if type_name.starts_with(|character: char| character.is_lowercase()) {
        return None;
    }
    let mut packages = modules
        .get(&file.path)
        .map(|declared| declared.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if packages.is_empty() {
        packages.push(String::new());
    }
    for import in file
        .symbols
        .iter()
        .filter(|import| import.role == SymbolRole::Definition && import.evidence == SymbolEvidence::Import)
    {
        let path = import
            .context
            .trim()
            .trim_start_matches("import ")
            .trim_start_matches("using ")
            .trim_end_matches(';')
            .trim();
        if path.contains(['=', ' ']) {
            continue;
        }
        if file.language == SourceLanguage::CSharp {
            packages.push(path.to_owned());
        } else if let Some(imported) = path.strip_suffix(".*") {
            packages.push(imported.to_owned());
        } else if let Some((imported, name)) = path.rsplit_once('.')
            && name == type_name
        {
            packages.push(imported.to_owned());
        }
    }
    packages.sort();
    packages.dedup();
    Some((type_name.to_owned(), packages))
}

fn is_type_declaration(symbol: &SourceSymbol) -> bool {
    symbol.role == SymbolRole::Definition
        && matches!(
            symbol.kind,
            SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface | SymbolKind::Enum
        )
}

/// Whether a Lua symbol assigns `table = {…}` outside any function and without
/// `local`, which makes `table` a global every loaded file can call through.
fn is_global_table(symbol: &SourceSymbol) -> bool {
    symbol.role == SymbolRole::Definition
        && symbol.kind == SymbolKind::Variable
        && symbol.visibility == SymbolVisibility::Public
        && symbol
            .context
            .split_once('=')
            .is_some_and(|(_, value)| value.trim_start().starts_with('{'))
}

/// Whether a Lua file declares `name` with `local`, such as `local M = {}` or
/// `local M = require("a")`, which hides any global `M`.
fn binds_local(file: &SourceFile, name: &str) -> bool {
    file.symbols.iter().any(|symbol| {
        symbol.role == SymbolRole::Definition
            && symbol.kind == SymbolKind::Variable
            && symbol.name == name
            && symbol.visibility != SymbolVisibility::Public
    })
}

/// Whether a Lua file defines `function table.name()` or `table.name = function`.
fn defines_table_function(file: &SourceFile, table: &str, name: &str) -> bool {
    file.symbols.iter().any(|symbol| {
        symbol.role == SymbolRole::Definition
            && symbol.kind == SymbolKind::Function
            && symbol.name == name
            && (symbol.scope.first().is_some_and(|scope| scope == table)
                || symbol.context.trim_start().starts_with(&format!("{table}.{name}")))
    })
}

/// Keeps the candidates whose module path ends with the reference's qualifier,
/// so `map::language_capabilities()` resolves to `map.rs` or `map/mod.rs`.
/// `crate`, `self` and `super` segments are dropped; a qualifier made only of
/// them names no module and yields no candidates. Only candidates in the
/// caller's crate count, as `crate_source_root` gives it.
///
/// A qualifier with no `crate`, `self` or `super` segment, such as `map::f`,
/// resolves only when its first segment is a module declared in the crate
/// (`mod map;`) or a `use` binding from inside the crate. A name the file
/// imports from another crate, such as `fs` after `use std::fs;`, and an extern
/// crate path such as `serde_json::from_str` yield no candidates.
fn qualified_path_candidates(
    file: &SourceFile, symbol: &SourceSymbol, candidates: &[(String, SymbolVisibility)], scope: &QualifierScope<'_>,
) -> Option<Vec<(String, SymbolVisibility)>> {
    let qualifier = symbol.qualifier.as_deref()?;
    let segments = qualifier
        .split("::")
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let anchored = segments
        .iter()
        .any(|segment| matches!(*segment, "crate" | "self" | "super"));
    if !anchored {
        let head = segments.first()?;
        let declared = scope
            .crate_modules
            .get(crate_source_root(&file.path))
            .is_some_and(|names| names.contains(*head));
        if !declared && !use_binding_root(file, head).is_some_and(is_crate_relative_root) {
            return None;
        }
    } else if qualifier_names_external_import(file, &segments) {
        return None;
    }
    let module = segments
        .iter()
        .filter(|segment| !matches!(**segment, "crate" | "self" | "super"))
        .copied()
        .collect::<Vec<_>>()
        .join("/")
        .to_ascii_lowercase();
    if module.is_empty() {
        return None;
    }
    let matching = candidates
        .iter()
        .filter(|(path, _)| {
            let normalized = normalize_module_hint(path);
            // `normalize_module_hint` strips `.rs` after `/mod`, so `map/mod.rs`
            // arrives here as `map/mod`.
            let normalized = normalized.strip_suffix("/mod").unwrap_or(&normalized);
            same_crate(&file.path, path) && (normalized == module || normalized.ends_with(&format!("/{module}")))
        })
        .cloned()
        .collect::<Vec<_>>();
    (!matching.is_empty()).then_some(matching)
}

/// Whether the qualifier's first segment after `self` is bound by a `use`
/// whose path starts outside the crate. Paths anchored at `crate::` or
/// `super::` do not read `use` bindings, so they never match.
fn qualifier_names_external_import(file: &SourceFile, segments: &[&str]) -> bool {
    if segments.iter().any(|segment| matches!(*segment, "crate" | "super")) {
        return false;
    }
    let Some(head) = segments.iter().find(|segment| **segment != "self") else {
        return false;
    };
    use_binding_root(file, head).is_some_and(|root| !is_crate_relative_root(root))
}

/// The root of the `use` path that binds `name` in the file, such as `std`
/// for `fs` after `use std::fs;`.
fn use_binding_root<'a>(file: &'a SourceFile, name: &str) -> Option<&'a str> {
    file.symbols
        .iter()
        .filter(|import| {
            import.role == SymbolRole::Definition && import.evidence == SymbolEvidence::Import && import.name == name
        })
        .find_map(|import| use_path_root(&import.context))
}

fn is_crate_relative_root(root: &str) -> bool {
    matches!(root, "crate" | "self" | "super")
}

/// Whether `candidate` is in the same crate as `source`: under the source's
/// path up to and including its last `src` directory, or, for a source with
/// no `src` directory, under the same top-level directory. A source at the
/// repository root matches only other files at the root.
fn same_crate(source: &str, candidate: &str) -> bool {
    let root = crate_source_root(source);
    if root.is_empty() { !candidate.contains('/') } else { candidate.starts_with(root) }
}

/// The prefix that marks a file's crate, ending in `/`: `crates/a/src/` for
/// `crates/a/src/map/mod.rs`, `tests/` for `tests/cli.rs`, and empty for a
/// file at the repository root.
pub(super) fn crate_source_root(path: &str) -> &str {
    let parent = repository_parent(path);
    let mut src_end = None;
    let mut offset = 0;
    for segment in parent.split('/') {
        offset += segment.len();
        if segment == "src" {
            src_end = Some(offset + 1);
        }
        offset += 1;
    }
    if let Some(end) = src_end {
        return &path[..end];
    }
    parent.split_once('/').map_or_else(
        || if parent.is_empty() { "" } else { &path[..parent.len() + 1] },
        |(top, _)| &path[..top.len() + 1],
    )
}

/// Returns the first segment of a Rust `use` path, such as `std` in
/// `pub use std::fs;`.
fn use_path_root(context: &str) -> Option<&str> {
    let (_, path) = context.split_once("use ")?;
    path.trim_start()
        .trim_start_matches("::")
        .split(|character: char| character == ':' || character == ';' || character == '{' || character.is_whitespace())
        .next()
        .filter(|root| !root.is_empty())
}

fn normalize_module_hint(value: &str) -> String {
    let value = value.trim_matches(['"', '\'', '`', ';', ',']);
    let value = value.trim_start_matches("./").trim_start_matches("../");
    value
        .replace('\\', "/")
        .trim_end_matches("/__init__")
        .trim_end_matches("/mod")
        .trim_end_matches(".js")
        .trim_end_matches(".ts")
        .trim_end_matches(".tsx")
        .trim_end_matches(".py")
        .trim_end_matches(".rb")
        .trim_end_matches(".rs")
        .trim_end_matches(".java")
        .trim_end_matches(".cs")
        .trim_end_matches(".go")
        .trim_end_matches(".lua")
        .trim_end_matches(".rockspec")
        .trim_end_matches(".zig")
        .replace("::", "/")
        .trim_matches('/')
        .to_ascii_lowercase()
}

fn module_path_matches(path: &str, hints: &[String]) -> bool {
    if hints.is_empty() {
        return false;
    }
    let normalized = normalize_module_hint(path);
    hints.iter().any(|hint| {
        let direct_match = normalized == *hint || normalized.ends_with(&format!("/{hint}"));
        let module = hint
            .rsplit_once('/')
            .map(|(module, _)| module)
            .unwrap_or(hint)
            .trim_start_matches("crate/")
            .trim_start_matches("self/")
            .trim_start_matches("super/");
        let imported_directory = hint.rsplit('/').next().unwrap_or(hint);
        let path_parent = repository_parent(&normalized);
        direct_match
            || normalized == module
            || normalized.ends_with(&format!("/{module}"))
            || path_parent == imported_directory
            || path_parent.ends_with(&format!("/{imported_directory}"))
    })
}

fn symbol_matches_focus(symbol: &SourceSymbol, focus: &str) -> bool {
    let focus = focus.trim().to_ascii_lowercase();
    !focus.is_empty()
        && (symbol.name.to_ascii_lowercase().contains(&focus) || symbol.context.to_ascii_lowercase().contains(&focus))
}

fn fit_snippet(candidate: &SnippetCandidate, budget: usize) -> Option<(SourceSymbol, usize, bool)> {
    let scope = if candidate.symbol.scope.is_empty() {
        "root".to_owned()
    } else {
        candidate.symbol.scope.join("::")
    };
    let prefix = format!(
        "{} {} {} {}:{}-{}:{} {}",
        candidate.path,
        candidate.symbol.kind.label(),
        candidate.symbol.name,
        candidate.symbol.location.start.line,
        candidate.symbol.location.start.column,
        candidate.symbol.location.end.line,
        candidate.symbol.location.end.column,
        scope
    );
    let full = format!("{prefix} {}", candidate.symbol.context);
    let full_cost = utils::token_count(&full);
    if full_cost <= budget {
        return Some((candidate.symbol.clone(), full_cost, false));
    }
    let marker = "…";
    if utils::token_count(&format!("{prefix} {marker}")) > budget {
        return None;
    }
    let max_chars = candidate.symbol.context.chars().count();
    let mut best = 0;
    for chars in 0..=max_chars {
        let context = candidate.symbol.context.chars().take(chars).collect::<String>();
        if utils::token_count(&format!("{prefix} {context}{marker}")) <= budget {
            best = chars;
        } else {
            break;
        }
    }
    let context = candidate.symbol.context.chars().take(best).collect::<String>();
    let mut symbol = candidate.symbol.clone();
    symbol.context = format!("{context}{marker}");
    let cost = utils::token_count(&format!("{prefix} {}", symbol.context));
    Some((symbol, cost, true))
}

fn lexical_weight(symbol: &str) -> f64 {
    if is_generic_name(symbol) || symbol.starts_with('_') { 0.25 } else { 1.0 }
}

fn edge_weight(edge: &LexicalEdge) -> f64 {
    let confidence = match edge.confidence {
        ConfidenceTier::High => 1.0,
        ConfidenceTier::Medium => 0.5,
        ConfidenceTier::Low => 0.25,
    };
    let visibility = match edge.target_visibility {
        SymbolVisibility::Public => 1.0,
        SymbolVisibility::Internal => 0.8,
        SymbolVisibility::Private => 0.35,
        SymbolVisibility::Unknown => 0.7,
    };
    lexical_weight(&edge.symbol) * confidence * visibility
}

fn is_generic_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "data" | "default" | "error" | "item" | "key" | "main" | "new" | "result" | "self" | "value"
    )
}

fn file_matches_focus(file: &SourceFile, focus: &str) -> bool {
    let focus = focus.trim().to_ascii_lowercase();
    !focus.is_empty()
        && (file.path.to_ascii_lowercase().contains(&focus)
            || file.symbols.iter().any(|symbol| {
                symbol.name.to_ascii_lowercase().contains(&focus)
                    || symbol.context.to_ascii_lowercase().contains(&focus)
            }))
}

fn path_matches_focus(path: &str, focus_path: &str) -> bool {
    let focus_path = focus_path.trim().replace('\\', "/");
    let focus_path = focus_path.trim_start_matches("./");
    !focus_path.is_empty() && (path == focus_path || path.starts_with(&format!("{focus_path}/")))
}

fn scaled_score(score: f64) -> u64 {
    (score.max(0.0) * RANK_SCALE).round() as u64
}

fn same_module_candidates(
    source: &SourceFile, candidates: &[(String, SymbolVisibility)], modules: &BTreeMap<String, BTreeSet<String>>,
) -> Option<Vec<(String, SymbolVisibility)>> {
    if source.language != SourceLanguage::Go {
        return None;
    }
    let source_modules = modules.get(&source.path)?;
    if source_modules.is_empty() {
        return None;
    }
    let source_parent = repository_parent(&source.path);
    let matches = candidates
        .iter()
        .filter(|(path, _)| {
            path != &source.path
                && repository_parent(path) == source_parent
                && modules
                    .get(path)
                    .is_some_and(|target_modules| !source_modules.is_disjoint(target_modules))
        })
        .cloned()
        .collect::<Vec<_>>();
    (!matches.is_empty()).then_some(matches)
}

fn repository_parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}
