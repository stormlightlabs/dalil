use std::{fmt::Write, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    AnalysisProfile, AnalysisRequest, CommandDescriptor, HistoryReport, Landmark, LandmarkKind, LexicalEdge, MapReport,
    ProjectRoot, ReportQuality, SourceFile, SourceSymbol,
};

/// Compatibility version for repository evidence bundles.
pub const EVIDENCE_MAP_SCHEMA_VERSION: u16 = 1;

/// A portable, task-independent repository evidence snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceMap {
    pub schema_version: u16,
    pub producer_version: String,
    pub snapshot_id: String,
    pub repository: crate::RepositoryIdentity,
    pub revision: crate::HeadSnapshot,
    pub worktree_fingerprint: String,
    pub scope: String,
    pub projects: Vec<EvidenceProject>,
    pub files: Vec<EvidenceFile>,
    pub omissions: Vec<crate::SourceOmission>,
    pub symbols: Vec<EvidenceSymbol>,
    pub relationships: Vec<EvidenceRelationship>,
    pub landmarks: Vec<EvidenceLandmark>,
    pub tests: Vec<EvidenceLandmark>,
    pub history: HistoryReport,
    pub quality: ReportQuality,
    pub limitations: Vec<String>,
    pub provenance: EvidenceMapProvenance,
    pub collections: EvidenceMapCollections,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceProject {
    pub id: String,
    #[serde(flatten)]
    pub project: ProjectRoot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceFile {
    pub id: String,
    #[serde(flatten)]
    pub file: SourceFile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceSymbol {
    pub id: String,
    pub path: String,
    #[serde(flatten)]
    pub symbol: SourceSymbol,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceRelationship {
    pub id: String,
    #[serde(flatten)]
    pub relationship: LexicalEdge,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceLandmark {
    pub id: String,
    #[serde(flatten)]
    pub landmark: Landmark,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceMapProvenance {
    pub captured_at: String,
    pub reference_time: String,
    pub languages: std::collections::BTreeMap<String, crate::LanguageProvenance>,
    pub cache: crate::CacheProvenance,
    pub path_encoding: crate::PathEncodingPolicy,
    pub history: crate::HistoryProvenance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceMapCollections {
    pub projects: crate::CollectionSummary,
    pub files: crate::CollectionSummary,
    pub omissions: crate::CollectionSummary,
    pub symbols: crate::CollectionSummary,
    pub relationships: crate::CollectionSummary,
    pub landmarks: crate::CollectionSummary,
    pub tests: crate::CollectionSummary,
    pub history: crate::HistoryCollections,
}

/// Analyze a repository and project its task-independent evidence into a portable map.
pub fn export(mut request: AnalysisRequest) -> Result<EvidenceMap, crate::CoreError> {
    request.command = CommandDescriptor {
        name: crate::CommandName::Briefing,
        operation: None,
        target: None,
        path: request.command.path,
    };
    request.profile = AnalysisProfile::Evidence;
    request.map.profile = AnalysisProfile::Evidence;
    request.map.task_seeds = Default::default();
    request.map.focuses.clear();
    request.map.focus_paths.clear();
    if !request.map.excludes.iter().any(|exclude| exclude == ".dalil/**") {
        request.map.excludes.push(".dalil/**".to_owned());
    }

    let report = crate::analyze(request)?;
    let map = report.map.expect("briefing analysis includes a source map");
    let history = report.history.expect("briefing analysis includes history");
    Ok(project(
        &report.provenance,
        report.quality,
        report.limitations,
        map,
        history,
    ))
}

fn project(
    provenance: &crate::ReportProvenance, quality: ReportQuality, report_limitations: Vec<crate::Limitation>,
    map: MapReport, history: HistoryReport,
) -> EvidenceMap {
    let mut files = map
        .files
        .into_iter()
        .map(|file| EvidenceFile { id: stable_id("file", &file), file })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.file.path.cmp(&right.file.path));

    let mut omissions = map.omissions;
    omissions.retain(|omission| !omission.path.starts_with(".dalil/"));
    omissions.sort_by(|left, right| left.path.cmp(&right.path));
    let mut collections = map.collections.clone();
    collections.omissions = crate::CollectionSummary::complete(omissions.len());

    let mut symbols = files
        .iter()
        .flat_map(|file| {
            file.file.symbols.iter().cloned().map(move |symbol| EvidenceSymbol {
                id: stable_id("symbol", &(file.file.path.as_str(), &symbol)),
                path: file.file.path.clone(),
                symbol,
            })
        })
        .collect::<Vec<_>>();
    symbols.sort_by(|left, right| left.id.cmp(&right.id));
    // The v1 file shape retains its `symbols` field for compatibility, while
    // the canonical symbol records live in the top-level collection.
    for file in &mut files {
        file.file.symbols.clear();
    }

    let mut relationships = map
        .edges
        .into_iter()
        .map(|relationship| EvidenceRelationship { id: stable_id("relationship", &relationship), relationship })
        .collect::<Vec<_>>();
    relationships.sort_by(|left, right| left.id.cmp(&right.id));

    let mut landmarks = map
        .landmarks
        .into_iter()
        .map(|landmark| EvidenceLandmark { id: stable_id("landmark", &landmark), landmark })
        .collect::<Vec<_>>();
    landmarks.sort_by(|left, right| left.id.cmp(&right.id));
    let tests = landmarks
        .iter()
        .filter(|landmark| landmark.landmark.kind == LandmarkKind::TestRoot)
        .cloned()
        .collect::<Vec<_>>();

    let mut projects = map
        .project_roots
        .into_iter()
        .map(|project| EvidenceProject { id: stable_id("project", &project), project })
        .collect::<Vec<_>>();
    projects.sort_by(|left, right| left.project.path.cmp(&right.project.path));
    let limitations = map
        .limitations
        .into_iter()
        .chain(history.limitations.iter().cloned())
        .chain(report_limitations.into_iter().map(|limitation| limitation.detail))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let worktree_fingerprint = fingerprint_worktree(
        Path::new(&provenance.repository.canonical_root),
        &provenance.repository.stable_id,
        &provenance.head,
        &files,
        &omissions,
    );
    let snapshot_id = stable_id(
        "snapshot",
        &(
            &provenance.repository.stable_id,
            &provenance.head,
            &worktree_fingerprint,
            &projects,
            &symbols,
            &relationships,
            &landmarks,
            &history,
            &quality,
            &limitations,
        ),
    );

    EvidenceMap {
        schema_version: EVIDENCE_MAP_SCHEMA_VERSION,
        producer_version: provenance.tool_version.clone(),
        snapshot_id,
        repository: provenance.repository.clone(),
        revision: provenance.head.clone(),
        worktree_fingerprint,
        scope: map.scope_path,
        collections: EvidenceMapCollections {
            projects: collections.project_roots,
            files: collections.files,
            omissions: collections.omissions,
            symbols: collections.symbols,
            relationships: collections.edges,
            landmarks: collections.landmarks,
            tests: crate::CollectionSummary::complete(tests.len()),
            history: history.collections.clone(),
        },
        projects,
        files,
        omissions,
        symbols,
        relationships,
        landmarks,
        tests,
        history,
        quality,
        limitations,
        provenance: EvidenceMapProvenance {
            captured_at: provenance.captured_at.clone(),
            reference_time: provenance.reference_time.clone(),
            languages: provenance.languages.clone(),
            cache: provenance.cache.clone(),
            path_encoding: provenance.path_encoding.clone(),
            history: provenance.history.clone().unwrap_or_default(),
        },
    }
}

fn fingerprint_worktree(
    root: &Path, repository_id: &str, head: &crate::HeadSnapshot, files: &[EvidenceFile],
    omissions: &[crate::SourceOmission],
) -> String {
    const MAX_FINGERPRINT_FILE_BYTES: usize = 1024 * 1024;

    let mut paths = files
        .iter()
        .map(|file| file.file.path.clone())
        .chain(omissions.iter().map(|omission| omission.path.clone()))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    let fingerprints = paths
        .into_iter()
        .map(|path| {
            let fingerprint =
                match crate::security::read_worktree_file_limited(root, root, &path, MAX_FINGERPRINT_FILE_BYTES) {
                    Ok(bytes) => format!("sha256:{}", hex_digest(Sha256::digest(bytes))),
                    Err(error) => format!("unavailable:{error}"),
                };
            (path, fingerprint)
        })
        .collect::<Vec<_>>();
    stable_id("worktree", &(repository_id, head, fingerprints))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let mut output = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        write!(&mut output, "{byte:02x}").expect("writing a string cannot fail");
    }
    output
}

fn stable_id<T: Serialize>(kind: &str, value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("evidence map facts are serializable");
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(kind.len() + 17);
    output.push_str(kind);
    output.push(':');
    for byte in digest.iter().take(8) {
        write!(&mut output, "{byte:02x}").expect("writing a string cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_are_repeatable() {
        assert_eq!(stable_id("file", &"src/lib.rs"), stable_id("file", &"src/lib.rs"));
        assert_ne!(stable_id("file", &"src/lib.rs"), stable_id("file", &"src/main.rs"));
    }
}
