use super::*;

#[derive(Debug, Deserialize, Serialize)]
struct CacheRecord {
    schema_version: u16,
    tool_version: String,
    repository_root: String,
    path: String,
    language: SourceLanguage,
    query_pack: String,
    query_digest: String,
    fingerprint: String,
    created_at: u128,
    parsed: ParsedSource,
}

#[derive(Debug, Deserialize, Serialize)]
struct RepositoryIndex {
    schema_version: u16,
    tool_version: String,
    repository_root: String,
    worktree_id: String,
    scope_path: String,
    head: Option<String>,
    worktree_state: String,
    manifest_state: String,
    profile: AnalysisProfile,
    map_tokens: usize,
    analysis_options: String,
    content_state: String,
    language_pack_identity: BTreeMap<String, String>,
    query_packs: BTreeMap<String, String>,
    files: Vec<IndexedFile>,
    lexical_edges: Vec<LexicalEdge>,
    history_path_weights: BTreeMap<String, u64>,
    created_at: u128,
}

#[derive(Debug, Deserialize, Serialize)]
struct IndexedFile {
    path: String,
    language: SourceLanguage,
    fingerprint: String,
    query_digest: String,
    symbols: Vec<IndexedSymbol>,
    status: FileAnalysisStatus,
}

#[derive(Debug, Deserialize, Serialize)]
struct IndexedSymbol {
    name: String,
    kind: SymbolKind,
    role: SymbolRole,
    scope: Vec<String>,
    location: SourceLocation,
    visibility: SymbolVisibility,
    evidence: SymbolEvidence,
}

#[derive(Clone, Debug)]
pub struct IndexFileInput {
    pub path: String,
    pub language: SourceLanguage,
    pub fingerprint: String,
    pub symbols: Vec<SourceSymbol>,
    pub status: FileAnalysisStatus,
}

#[derive(Clone, Debug)]
pub struct IndexIdentity {
    pub scope_path: String,
    pub head: Option<String>,
    pub worktree_state: String,
    pub manifest_state: String,
    pub profile: AnalysisProfile,
    pub map_tokens: usize,
    pub analysis_options: String,
    pub content_state: String,
    pub language_pack_identity: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct IndexState {
    pub status: PersistentIndexStatus,
    pub detail: Option<String>,
    index: Option<RepositoryIndex>,
}

#[derive(Debug)]
pub struct CacheStore {
    pub root: Option<PathBuf>,
    pub repository_root: String,
    pub repository_id: String,
}

impl CacheStore {
    pub fn new(repository_root: &Path) -> Result<Self> {
        let root = security::cache_root(repository_root)
            .map_err(|error| MapError::safety("resolving the cache root", error))?;
        let repository_root = repository_root.to_string_lossy().into_owned();
        Ok(Self { root, repository_id: digest_hex(repository_root.as_bytes()), repository_root })
    }

    fn repository_path(&self) -> Option<PathBuf> {
        self.root
            .as_ref()
            .map(|root| root.join("repositories").join(&self.repository_id))
    }

    fn index_path(&self) -> Option<PathBuf> {
        Some(self.repository_path()?.join("index-v1.index"))
    }

    pub fn load_index(&self, identity: &IndexIdentity) -> IndexState {
        let Some(root) = self.root.as_ref() else {
            return IndexState { status: PersistentIndexStatus::Bypassed, detail: None, index: None };
        };
        let Some(path) = self.index_path() else {
            return IndexState { status: PersistentIndexStatus::Bypassed, detail: None, index: None };
        };
        if !path.is_file() {
            return IndexState { status: PersistentIndexStatus::Missing, detail: None, index: None };
        }
        let bytes = match security::read_cache_file(root, &path) {
            Ok(bytes) => bytes,
            Err(error) => {
                return IndexState {
                    status: PersistentIndexStatus::Failed,
                    detail: Some(format!("could not read the repository index: {error}")),
                    index: None,
                };
            }
        };
        let index: RepositoryIndex = match serde_json::from_slice(&bytes) {
            Ok(index) => index,
            Err(_) => {
                return IndexState {
                    status: PersistentIndexStatus::Corrupt,
                    detail: Some("the repository index was not valid JSON and will be replaced".to_owned()),
                    index: None,
                };
            }
        };
        if index.schema_version != CACHE_INDEX_SCHEMA_VERSION
            || index.tool_version != CACHE_TOOL_VERSION
            || index.repository_root != self.repository_root
            || index.worktree_id != self.repository_id
            || index.scope_path != identity.scope_path
            || index.profile != identity.profile
            || index.map_tokens != identity.map_tokens
            || index.analysis_options != identity.analysis_options
            || index.language_pack_identity != identity.language_pack_identity
        {
            return IndexState {
                status: PersistentIndexStatus::Incompatible,
                detail: Some("the repository index was created for incompatible analysis options".to_owned()),
                index: None,
            };
        }
        let stale = index.head != identity.head
            || index.worktree_state != identity.worktree_state
            || index.manifest_state != identity.manifest_state
            || index.content_state != identity.content_state;
        let detail = stale.then(|| index_invalidation_detail(&index, identity));
        IndexState {
            status: if stale { PersistentIndexStatus::Stale } else { PersistentIndexStatus::Hit },
            detail,
            index: Some(index),
        }
    }

    pub fn write_index(
        &self, identity: &IndexIdentity, query_packs: BTreeMap<String, String>, files: Vec<IndexFileInput>,
        lexical_edges: Vec<LexicalEdge>, history_path_weights: BTreeMap<String, u64>,
    ) -> Option<String> {
        let path = self.index_path()?;
        let index = RepositoryIndex {
            schema_version: CACHE_INDEX_SCHEMA_VERSION,
            tool_version: CACHE_TOOL_VERSION.to_owned(),
            repository_root: self.repository_root.clone(),
            worktree_id: self.repository_id.clone(),
            scope_path: identity.scope_path.clone(),
            head: identity.head.clone(),
            worktree_state: identity.worktree_state.clone(),
            manifest_state: identity.manifest_state.clone(),
            profile: identity.profile,
            map_tokens: identity.map_tokens,
            analysis_options: identity.analysis_options.clone(),
            content_state: identity.content_state.clone(),
            language_pack_identity: identity.language_pack_identity.clone(),
            query_packs,
            files: files
                .into_iter()
                .map(|file| IndexedFile {
                    query_digest: support_for_path(Path::new(&file.path)).map_or_else(String::new, query_digest),
                    symbols: file
                        .symbols
                        .into_iter()
                        .map(|symbol| IndexedSymbol {
                            name: symbol.name,
                            kind: symbol.kind,
                            role: symbol.role,
                            scope: symbol.scope,
                            location: symbol.location,
                            visibility: symbol.visibility,
                            evidence: symbol.evidence,
                        })
                        .collect(),
                    path: file.path,
                    language: file.language,
                    fingerprint: file.fingerprint,
                    status: file.status,
                })
                .collect(),
            lexical_edges,
            history_path_weights,
            created_at: unix_timestamp(),
        };
        let bytes = match serde_json::to_vec(&index) {
            Ok(bytes) => bytes,
            Err(error) => return Some(format!("could not serialize the repository index: {error}")),
        };
        let root = self.root.as_ref()?;
        if let Err(error) = security::write_cache_file(root, &path, &bytes) {
            return Some(format!("could not write the repository index: {error}"));
        }
        self.prune_repository()
            .err()
            .map(|error| format!("could not prune repository index records: {error}"))
    }

    fn record_directory(&self, path: &str, support: &LanguageSupport) -> Option<PathBuf> {
        let root = self.root.as_ref()?;
        let identity = format!(
            "{CACHE_TOOL_VERSION}\n{}\n{}\n{}\n{}",
            self.repository_root,
            path,
            support.language.label(),
            query_digest(support),
        );
        Some(
            root.join("repositories")
                .join(&self.repository_id)
                .join("files")
                .join(digest_hex(identity.as_bytes())),
        )
    }

    fn record_path(&self, path: &str, support: &LanguageSupport, fingerprint: &str) -> Option<PathBuf> {
        Some(
            self.record_directory(path, support)?
                .join(format!("{fingerprint}.json")),
        )
    }

    pub fn load(
        &self, path: &str, support: &LanguageSupport, fingerprint: &str, mode: CacheMode,
    ) -> Option<CacheLookup> {
        let directory = self.record_directory(path, support)?;
        let root = self.root.as_ref()?;
        if security::cache_path_is_safe(root, &directory).is_err() {
            return None;
        }

        if mode != CacheMode::Manual {
            let current = self.record_path(path, support, fingerprint)?;
            let record = self.read_record(current, path, support)?;
            return (record.fingerprint == fingerprint).then_some(CacheLookup { parsed: record.parsed, stale: false });
        }

        let entries = fs::read_dir(directory).ok()?;
        let mut candidates = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|candidate| candidate.extension().is_some_and(|extension| extension == "json"))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            self.cache_record_created_at(right)
                .cmp(&self.cache_record_created_at(left))
                .then_with(|| cache_record_mtime(right).cmp(&cache_record_mtime(left)))
                .then_with(|| left.cmp(right))
        });
        candidates
            .into_iter()
            .filter(|candidate| security::cache_path_is_safe(root, candidate).is_ok())
            .find_map(|candidate| {
                let record = self.read_record(candidate, path, support)?;
                Some(CacheLookup { parsed: record.parsed, stale: record.fingerprint != fingerprint })
            })
    }

    fn read_record(&self, path: PathBuf, expected_path: &str, support: &LanguageSupport) -> Option<CacheRecord> {
        let bytes = self
            .root
            .as_ref()
            .and_then(|root| security::read_cache_file(root, &path).ok())?;
        let record: CacheRecord = serde_json::from_slice(&bytes).ok()?;
        if record.schema_version != CACHE_SCHEMA_VERSION
            || record.tool_version != CACHE_TOOL_VERSION
            || record.repository_root != self.repository_root
            || record.path != expected_path
            || record.language != support.language
            || record.query_pack != support.query_pack
            || record.query_digest != query_digest(support)
        {
            return None;
        }
        Some(record)
    }

    fn cache_record_created_at(&self, path: &Path) -> u128 {
        self.root
            .as_ref()
            .and_then(|root| security::read_cache_file(root, path).ok())
            .and_then(|bytes| serde_json::from_slice::<CacheRecord>(&bytes).ok())
            .map(|record| record.created_at)
            .unwrap_or_default()
    }

    pub fn write(
        &self, path: &str, support: &LanguageSupport, fingerprint: &str, parsed: &ParsedSource,
    ) -> Option<String> {
        let record_path = self.record_path(path, support, fingerprint)?;
        let record = CacheRecord {
            schema_version: CACHE_SCHEMA_VERSION,
            tool_version: CACHE_TOOL_VERSION.to_owned(),
            repository_root: self.repository_root.clone(),
            path: path.to_owned(),
            language: support.language,
            query_pack: support.query_pack.to_owned(),
            query_digest: query_digest(support),
            fingerprint: fingerprint.to_owned(),
            created_at: unix_timestamp(),
            parsed: parsed.clone(),
        };
        let bytes = match serde_json::to_vec(&record) {
            Ok(bytes) => bytes,
            Err(error) => return Some(format!("could not serialize the source-map cache record: {error}")),
        };
        let root = self.root.as_ref()?;
        if let Err(error) = security::write_cache_file(root, &record_path, &bytes) {
            return Some(format!("could not write the source-map cache record: {error}"));
        }
        self.prune_repository()
            .err()
            .map(|error| format!("could not prune source-map cache records: {error}"))
    }

    fn prune_repository(&self) -> std::result::Result<(usize, u64), String> {
        let Some(root) = self.root.as_ref() else {
            return Ok((0, 0));
        };
        let Some(repository_path) = self.repository_path() else {
            return Ok((0, 0));
        };
        prune_cache_directory(root, &repository_path)
    }
}

fn index_invalidation_detail(index: &RepositoryIndex, identity: &IndexIdentity) -> String {
    let mut causes = Vec::new();
    if index.head != identity.head {
        causes.push("repository revision changed");
    }
    if index.worktree_state != identity.worktree_state {
        causes.push("worktree inventory changed");
    }
    if index.manifest_state != identity.manifest_state {
        causes.push("manifest content changed");
    }
    if index.content_state != identity.content_state {
        causes.push("source file fingerprints changed");
    }
    format!("the repository index was invalidated because {}", causes.join(", "))
}

pub struct IncrementalEdges {
    pub edges: Vec<LexicalEdge>,
    pub invalidated: Vec<String>,
}

impl IndexState {
    /// Rebuild only the lexical component that could be affected by changed
    /// source fingerprints. Any bounded graph that might have been cut off is
    /// rebuilt cold so the result remains equivalent to a fresh analysis.
    pub fn incremental_edges(
        &self, files: &[SourceFile], fingerprints: &BTreeMap<String, String>, max_candidates: usize, max_edges: usize,
    ) -> Option<IncrementalEdges> {
        let index = self.index.as_ref()?;
        if index.lexical_edges.len() >= max_edges {
            return None;
        }

        let current = files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect::<BTreeMap<_, _>>();
        let previous = index
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect::<BTreeMap<_, _>>();
        let paths = current.keys().chain(previous.keys()).copied().collect::<BTreeSet<_>>();
        let mut invalidated = paths
            .into_iter()
            .filter(|path| match (current.get(path), previous.get(path)) {
                (Some(current), Some(previous)) => {
                    current.language != previous.language
                        || fingerprints
                            .get(*path)
                            .is_none_or(|fingerprint| fingerprint != &previous.fingerprint)
                }
                _ => true,
            })
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        if invalidated.is_empty() {
            return Some(IncrementalEdges { edges: index.lexical_edges.clone(), invalidated: Vec::new() });
        }

        let mut changed_names = BTreeSet::new();
        for path in &invalidated {
            if let Some(file) = current.get(path.as_str()) {
                changed_names.extend(
                    file.symbols
                        .iter()
                        .filter(|symbol| symbol.role == SymbolRole::Definition)
                        .map(|symbol| (file.language, symbol.name.clone())),
                );
            }
            if let Some(file) = previous.get(path.as_str()) {
                changed_names.extend(
                    file.symbols
                        .iter()
                        .filter(|symbol| symbol.role == SymbolRole::Definition)
                        .map(|symbol| (file.language, symbol.name.clone())),
                );
            }
        }

        let mut affected = invalidated.clone();
        for file in files {
            if file.symbols.iter().any(|symbol| {
                symbol.role == SymbolRole::Reference && changed_names.contains(&(file.language, symbol.name.clone()))
            }) {
                affected.insert(file.path.clone());
            }
        }
        loop {
            let mut expanded = false;
            let active_paths = affected.clone();
            for file in files.iter().filter(|file| active_paths.contains(&file.path)) {
                for reference in file
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.role == SymbolRole::Reference)
                {
                    for definition_file in files.iter().filter(|candidate| {
                        candidate.language == file.language
                            && candidate
                                .symbols
                                .iter()
                                .any(|symbol| symbol.role == SymbolRole::Definition && symbol.name == reference.name)
                    }) {
                        expanded |= affected.insert(definition_file.path.clone());
                    }
                }
            }
            if !expanded {
                break;
            }
        }

        let mut edges = index
            .lexical_edges
            .iter()
            .filter(|edge| !affected.contains(&edge.source) && !affected.contains(&edge.target))
            .cloned()
            .collect::<Vec<_>>();
        let affected_files = files
            .iter()
            .filter(|file| affected.contains(&file.path))
            .cloned()
            .collect::<Vec<_>>();
        edges.extend(build_lexical_edges(&affected_files, max_candidates, max_edges));
        if edges.len() > max_edges {
            return None;
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
        Some(IncrementalEdges { edges, invalidated: std::mem::take(&mut invalidated).into_iter().collect() })
    }
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub index_status: PersistentIndexStatus,
    pub index_detail: Option<String>,
    pub matched: usize,
    pub unmatched: usize,
    pub unavailable: usize,
    pub reused: Vec<String>,
    pub invalidated: Vec<String>,
    pub hits: usize,
    pub misses: usize,
    pub refreshed: Vec<String>,
    pub stale: Vec<String>,
}

#[derive(Debug)]
pub struct CacheLookup {
    pub parsed: ParsedSource,
    pub stale: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheCommand {
    Path,
    Status,
    Prune,
    Clear,
}

#[derive(Clone, Debug, Serialize)]
pub struct CacheControlReport {
    pub operation: String,
    pub path: Option<String>,
    pub exists: bool,
    pub repositories: usize,
    pub records: usize,
    pub bytes: u64,
    pub removed_records: usize,
    pub removed_bytes: u64,
    pub max_records_per_repository: usize,
    pub max_bytes_per_repository: u64,
    pub max_age_seconds: u64,
}

#[derive(Debug)]
pub struct CacheFileInfo {
    path: PathBuf,
    bytes: u64,
    timestamp: u128,
    repository_id: Option<String>,
}

/// Inspect or mutate only Dalil's configured cache root.
pub fn cache_control(command: CacheCommand) -> Result<CacheControlReport> {
    let root =
        security::configured_cache_root().map_err(|error| MapError::safety("resolving the cache root", error))?;
    let operation = match command {
        CacheCommand::Path => "path",
        CacheCommand::Status => "status",
        CacheCommand::Prune => "prune",
        CacheCommand::Clear => "clear",
    };
    let Some(root) = root else {
        return Ok(CacheControlReport {
            operation: operation.to_owned(),
            path: None,
            exists: false,
            repositories: 0,
            records: 0,
            bytes: 0,
            removed_records: 0,
            removed_bytes: 0,
            max_records_per_repository: CACHE_MAX_RECORDS_PER_REPOSITORY,
            max_bytes_per_repository: CACHE_MAX_BYTES_PER_REPOSITORY,
            max_age_seconds: CACHE_MAX_AGE_SECONDS,
        });
    };

    let exists = root.is_dir();
    let mut removed_records = 0;
    let mut removed_bytes = 0;
    if exists && command == CacheCommand::Prune {
        let repositories = root.join("repositories");
        for repository in direct_directories(&repositories)? {
            let (removed, bytes) = prune_cache_directory(&root, &repository)
                .map_err(|error| MapError::analysis("pruning the cache", error))?;
            removed_records += removed;
            removed_bytes += bytes;
        }
    } else if exists && command == CacheCommand::Clear {
        let repositories = root.join("repositories");
        for repository in direct_directories(&repositories)? {
            if security::cache_path_is_safe(&root, &repository).is_err() {
                continue;
            }
            let info = collect_cache_files(&root, &repository)?;
            removed_records += info.len();
            removed_bytes += info.iter().map(|entry| entry.bytes).sum::<u64>();
            fs::remove_dir_all(repository).map_err(|error| MapError::analysis("clearing the cache", error))?;
        }
    }

    let files = if root.is_dir() { collect_cache_files(&root, &root.join("repositories"))? } else { Vec::new() };
    let repositories = files
        .iter()
        .filter_map(|entry| entry.repository_id.as_deref())
        .collect::<BTreeSet<_>>()
        .len();
    Ok(CacheControlReport {
        operation: operation.to_owned(),
        path: Some(root.to_string_lossy().into_owned()),
        exists,
        repositories,
        records: files.len(),
        bytes: files.iter().map(|entry| entry.bytes).sum(),
        removed_records,
        removed_bytes,
        max_records_per_repository: CACHE_MAX_RECORDS_PER_REPOSITORY,
        max_bytes_per_repository: CACHE_MAX_BYTES_PER_REPOSITORY,
        max_age_seconds: CACHE_MAX_AGE_SECONDS,
    })
}

pub fn collect_cache_files(root: &Path, directory: &Path) -> Result<Vec<CacheFileInfo>> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    if security::cache_path_is_safe(root, directory).is_err() {
        return Ok(Vec::new());
    }
    let repository_id = directory
        .strip_prefix(root.join("repositories"))
        .ok()
        .and_then(|relative| relative.components().next())
        .and_then(|component| component.as_os_str().to_str())
        .map(str::to_owned);
    let mut files = Vec::new();
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(MapError::analysis("reading the cache", error)),
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(MapError::analysis("reading the cache", error)),
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(MapError::analysis("reading the cache", error)),
        };
        let path = entry.path();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            files.extend(collect_cache_files(root, &path)?);
        } else if file_type.is_file()
            && (path.extension().is_some_and(|extension| extension == "json")
                || path.file_name().is_some_and(|name| name == "index-v1.index"))
        {
            if security::cache_path_is_safe(root, &path).is_err() {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(MapError::analysis("reading cache metadata", error)),
            };
            let timestamp = security::read_cache_file(root, &path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<CacheRecord>(&bytes).ok())
                .map(|record| record.created_at)
                .unwrap_or_else(|| {
                    metadata
                        .modified()
                        .ok()
                        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                        .map(|duration| duration.as_nanos())
                        .unwrap_or_default()
                });
            files.push(CacheFileInfo { path, bytes: metadata.len(), timestamp, repository_id: repository_id.clone() });
        }
    }
    Ok(files)
}

pub fn prune_cache_directory(root: &Path, repository: &Path) -> std::result::Result<(usize, u64), String> {
    let mut files = collect_cache_files(root, repository).map_err(|error| error.to_string())?;
    files.sort_by(|left, right| {
        right
            .timestamp
            .cmp(&left.timestamp)
            .then_with(|| left.path.cmp(&right.path))
    });
    let cutoff = unix_timestamp().saturating_sub(u128::from(CACHE_MAX_AGE_SECONDS) * 1_000_000_000);
    let mut remove = files
        .iter()
        .filter(|entry| entry.timestamp < cutoff)
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    let mut remaining = files
        .iter()
        .filter(|entry| !remove.contains(&entry.path))
        .collect::<Vec<_>>();
    let mut remaining_bytes = remaining.iter().map(|entry| entry.bytes).sum::<u64>();
    while remaining.len() > CACHE_MAX_RECORDS_PER_REPOSITORY || remaining_bytes > CACHE_MAX_BYTES_PER_REPOSITORY {
        let Some(entry) = remaining.pop() else {
            break;
        };
        remaining_bytes = remaining_bytes.saturating_sub(entry.bytes);
        remove.insert(entry.path.clone());
    }
    let mut removed_bytes = 0;
    for path in &remove {
        if security::cache_path_is_safe(root, path).is_err() {
            continue;
        }
        if let Ok(metadata) = fs::symlink_metadata(path) {
            removed_bytes += metadata.len();
        }
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok((remove.len(), removed_bytes))
}

fn direct_directories(path: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(MapError::analysis("reading the cache", error)),
    };
    let mut directories = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|file_type| file_type.is_dir() && !file_type.is_symlink())
                .map(|_| entry.path())
        })
        .collect::<Vec<_>>();
    directories.sort();
    Ok(directories)
}
