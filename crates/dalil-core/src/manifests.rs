use std::collections::BTreeSet;

use serde_json::Value as JsonValue;
use toml::Value as TomlValue;

use crate::report::{ManifestCommand, ManifestCommandKind, ManifestMetadata, ManifestTarget};
use crate::security;

const MAX_ITEMS_PER_KIND: usize = 16;
const MAX_VALUE_CHARS: usize = 512;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ManifestError {
    #[error("manifest is not UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("top-level JSON value is not an object")]
    JsonRootNotObject,
}

pub(crate) fn inspect(
    manifest_path: &str, basename: &str, bytes: &[u8], known_paths: &BTreeSet<String>,
) -> Result<Option<ManifestMetadata>, ManifestError> {
    match basename {
        "cargo.toml" => cargo_metadata(manifest_path, bytes, known_paths).map(Some),
        "package.json" => package_json_metadata(manifest_path, bytes, known_paths).map(Some),
        "pyproject.toml" => pyproject_metadata(manifest_path, bytes, known_paths).map(Some),
        _ => Ok(None),
    }
}

fn cargo_metadata(
    manifest_path: &str, bytes: &[u8], known_paths: &BTreeSet<String>,
) -> Result<ManifestMetadata, ManifestError> {
    let text = std::str::from_utf8(bytes)?;
    let value = text.parse::<TomlValue>()?;
    let root = manifest_root(manifest_path);
    let package = value.get("package").and_then(TomlValue::as_table);
    let package_name = package.and_then(|table| table.get("name")).and_then(TomlValue::as_str);
    let mut metadata = ManifestMetadata {
        path: manifest_path.to_owned(),
        truncated: false,
        runtime_entry_points: Vec::new(),
        library_exports: Vec::new(),
        commands: vec![
            command(ManifestCommandKind::Build, None, "cargo build"),
            command(ManifestCommandKind::Test, None, "cargo test"),
        ],
    };

    let autolib = package
        .and_then(|table| table.get("autolib"))
        .and_then(TomlValue::as_bool)
        .unwrap_or(true);
    if let Some(library) = value.get("lib").and_then(TomlValue::as_table) {
        let declared = library.get("path").and_then(TomlValue::as_str).unwrap_or("src/lib.rs");
        let name = library.get("name").and_then(TomlValue::as_str).or(package_name);
        push_target(
            &mut metadata.library_exports,
            target(name, declared, resolve_relative_path(&root, declared, known_paths)),
            &mut metadata.truncated,
        );
    } else if autolib && path_exists(&root, "src/lib.rs", known_paths) {
        push_target(
            &mut metadata.library_exports,
            target(
                package_name,
                "src/lib.rs",
                resolve_relative_path(&root, "src/lib.rs", known_paths),
            ),
            &mut metadata.truncated,
        );
    }

    let autobins = package
        .and_then(|table| table.get("autobins"))
        .and_then(TomlValue::as_bool)
        .unwrap_or(true);
    if autobins && path_exists(&root, "src/main.rs", known_paths) {
        push_target(
            &mut metadata.runtime_entry_points,
            target(
                package_name,
                "src/main.rs",
                resolve_relative_path(&root, "src/main.rs", known_paths),
            ),
            &mut metadata.truncated,
        );
    }
    if let Some(binaries) = value.get("bin").and_then(TomlValue::as_array) {
        for binary in binaries {
            let Some(table) = binary.as_table() else {
                continue;
            };
            let name = table.get("name").and_then(TomlValue::as_str);
            let declared = table.get("path").and_then(TomlValue::as_str);
            let resolved = declared
                .and_then(|path| resolve_relative_path(&root, path, known_paths))
                .or_else(|| name.and_then(|name| resolve_cargo_bin(&root, name, known_paths)));
            let declaration = declared
                .map(ToOwned::to_owned)
                .or_else(|| resolved.as_ref().and_then(|path| relative_to_root(&root, path)))
                .unwrap_or_else(|| name.unwrap_or("unnamed binary").to_owned());
            push_target(
                &mut metadata.runtime_entry_points,
                target_owned(name.map(ToOwned::to_owned), declaration, resolved),
                &mut metadata.truncated,
            );
        }
    }
    for entry in &metadata.runtime_entry_points {
        let Some(command_text) = entry
            .name
            .as_deref()
            .map(|name| portable_token(name).map(|name| format!("cargo run --bin {name}")))
            .unwrap_or_else(|| Some("cargo run".to_owned()))
        else {
            continue;
        };
        push_command(
            &mut metadata.commands,
            command(ManifestCommandKind::Run, entry.name.as_deref(), &command_text),
            &mut metadata.truncated,
        );
    }
    normalize(&mut metadata);
    Ok(metadata)
}

fn package_json_metadata(
    manifest_path: &str, bytes: &[u8], known_paths: &BTreeSet<String>,
) -> Result<ManifestMetadata, ManifestError> {
    let value: JsonValue = serde_json::from_slice(bytes)?;
    let object = value.as_object().ok_or(ManifestError::JsonRootNotObject)?;
    let root = manifest_root(manifest_path);
    let package_name = object.get("name").and_then(JsonValue::as_str);
    let mut metadata = ManifestMetadata {
        path: manifest_path.to_owned(),
        truncated: false,
        runtime_entry_points: Vec::new(),
        library_exports: Vec::new(),
        commands: Vec::new(),
    };

    match object.get("bin") {
        Some(JsonValue::String(path)) => push_target(
            &mut metadata.runtime_entry_points,
            target(package_name, path, resolve_relative_path(&root, path, known_paths)),
            &mut metadata.truncated,
        ),
        Some(JsonValue::Object(entries)) => {
            for (name, value) in entries {
                if let Some(path) = value.as_str() {
                    push_target(
                        &mut metadata.runtime_entry_points,
                        target(Some(name), path, resolve_relative_path(&root, path, known_paths)),
                        &mut metadata.truncated,
                    );
                }
            }
        }
        _ => {}
    }
    for field in ["main", "module"] {
        if let Some(path) = object.get(field).and_then(JsonValue::as_str) {
            push_target(
                &mut metadata.library_exports,
                target(Some(field), path, resolve_relative_path(&root, path, known_paths)),
                &mut metadata.truncated,
            );
        }
    }
    if let Some(exports) = object.get("exports") {
        collect_json_exports(
            exports,
            "exports",
            &root,
            known_paths,
            &mut metadata.library_exports,
            &mut metadata.truncated,
        );
    }
    for entry in &metadata.runtime_entry_points {
        if let Some(name) = entry.name.as_deref() {
            let Some(name) = portable_token(name) else {
                continue;
            };
            push_command(
                &mut metadata.commands,
                command(ManifestCommandKind::Run, Some(name), name),
                &mut metadata.truncated,
            );
        }
    }
    if let Some(scripts) = object.get("scripts").and_then(JsonValue::as_object) {
        let runner = object
            .get("packageManager")
            .and_then(JsonValue::as_str)
            .and_then(package_runner)
            .unwrap_or("npm");
        for name in scripts.keys() {
            let kind = if name == "build" || name.starts_with("build:") {
                Some(ManifestCommandKind::Build)
            } else if name == "test" || name.starts_with("test:") {
                Some(ManifestCommandKind::Test)
            } else if matches!(name.as_str(), "start" | "dev" | "serve" | "run") {
                Some(ManifestCommandKind::Run)
            } else {
                None
            };
            if let Some(kind) = kind {
                let Some(name_token) = portable_token(name) else {
                    continue;
                };
                let invocation = format!("{runner} run {name_token}");
                push_command(
                    &mut metadata.commands,
                    command(kind, Some(name), &invocation),
                    &mut metadata.truncated,
                );
            }
        }
    }
    normalize(&mut metadata);
    Ok(metadata)
}

fn pyproject_metadata(
    manifest_path: &str, bytes: &[u8], known_paths: &BTreeSet<String>,
) -> Result<ManifestMetadata, ManifestError> {
    let text = std::str::from_utf8(bytes)?;
    let value = text.parse::<TomlValue>()?;
    let root = manifest_root(manifest_path);
    let mut metadata = ManifestMetadata {
        path: manifest_path.to_owned(),
        truncated: false,
        runtime_entry_points: Vec::new(),
        library_exports: Vec::new(),
        commands: Vec::new(),
    };
    if value.get("build-system").is_some() {
        push_command(
            &mut metadata.commands,
            command(ManifestCommandKind::Build, None, "python -m build"),
            &mut metadata.truncated,
        );
    }
    if value.get("tool").and_then(|tool| tool.get("pytest")).is_some() {
        push_command(
            &mut metadata.commands,
            command(ManifestCommandKind::Test, None, "pytest"),
            &mut metadata.truncated,
        );
    }
    if let Some(project) = value.get("project").and_then(TomlValue::as_table) {
        for table_name in ["scripts", "gui-scripts"] {
            if let Some(entries) = project.get(table_name).and_then(TomlValue::as_table) {
                collect_python_scripts(entries, &root, known_paths, &mut metadata);
            }
        }
        if let Some(imports) = project.get("import-names").and_then(TomlValue::as_array) {
            for import_name in imports.iter().filter_map(TomlValue::as_str) {
                let public_name = import_name.split(';').next().unwrap_or(import_name).trim();
                if !public_name.is_empty() {
                    push_target(
                        &mut metadata.library_exports,
                        target(
                            Some(public_name),
                            public_name,
                            resolve_python_module(&root, public_name, known_paths),
                        ),
                        &mut metadata.truncated,
                    );
                }
            }
        }
    }
    if let Some(entries) = value
        .get("tool")
        .and_then(|tool| tool.get("poetry"))
        .and_then(|poetry| poetry.get("scripts"))
        .and_then(TomlValue::as_table)
    {
        collect_python_scripts(entries, &root, known_paths, &mut metadata);
    }
    normalize(&mut metadata);
    Ok(metadata)
}

fn collect_python_scripts(
    entries: &toml::map::Map<String, TomlValue>, root: &str, known_paths: &BTreeSet<String>,
    metadata: &mut ManifestMetadata,
) {
    for (name, value) in entries {
        let Some(reference) = value.as_str() else {
            continue;
        };
        let module = reference.split(':').next().unwrap_or(reference).trim();
        push_target(
            &mut metadata.runtime_entry_points,
            target(Some(name), reference, resolve_python_module(root, module, known_paths)),
            &mut metadata.truncated,
        );
        if let Some(command_name) = portable_token(name) {
            push_command(
                &mut metadata.commands,
                command(ManifestCommandKind::Run, Some(name), command_name),
                &mut metadata.truncated,
            );
        }
    }
}

fn collect_json_exports(
    value: &JsonValue, name: &str, root: &str, known_paths: &BTreeSet<String>, output: &mut Vec<ManifestTarget>,
    truncated: &mut bool,
) {
    if output.len() >= MAX_ITEMS_PER_KIND {
        *truncated = true;
        return;
    }
    match value {
        JsonValue::String(path) => {
            push_target(
                output,
                target(Some(name), path, resolve_relative_path(root, path, known_paths)),
                truncated,
            );
        }
        JsonValue::Array(values) => {
            for value in values {
                collect_json_exports(value, name, root, known_paths, output, truncated);
            }
        }
        JsonValue::Object(entries) => {
            for (key, value) in entries {
                let child_name = if key.starts_with('.') { key.to_owned() } else { format!("{name}:{key}") };
                collect_json_exports(value, &child_name, root, known_paths, output, truncated);
            }
        }
        _ => {}
    }
}

fn resolve_cargo_bin(root: &str, name: &str, known_paths: &BTreeSet<String>) -> Option<String> {
    [format!("src/bin/{name}.rs"), format!("src/bin/{name}/main.rs")]
        .into_iter()
        .find_map(|path| resolve_relative_path(root, &path, known_paths))
}

fn resolve_python_module(root: &str, module: &str, known_paths: &BTreeSet<String>) -> Option<String> {
    if module.is_empty() || module.contains('/') || module.contains('\\') {
        return None;
    }
    let module_path = module.replace('.', "/");
    [
        format!("{module_path}.py"),
        format!("{module_path}/__init__.py"),
        format!("src/{module_path}.py"),
        format!("src/{module_path}/__init__.py"),
    ]
    .into_iter()
    .find_map(|path| resolve_relative_path(root, &path, known_paths))
}

fn resolve_relative_path(root: &str, declared: &str, known_paths: &BTreeSet<String>) -> Option<String> {
    let declared = declared.trim().trim_start_matches("./");
    if declared.is_empty() || declared.len() > MAX_VALUE_CHARS {
        return None;
    }
    let relative = if root == "." { declared.to_owned() } else { format!("{root}/{declared}") };
    let validated = security::validate_repository_path(relative.as_bytes()).ok()?;
    known_paths.contains(&validated).then_some(validated)
}

fn path_exists(root: &str, path: &str, known_paths: &BTreeSet<String>) -> bool {
    resolve_relative_path(root, path, known_paths).is_some()
}

fn manifest_root(path: &str) -> String {
    path.rsplit_once('/')
        .map_or_else(|| ".".to_owned(), |(root, _)| root.to_owned())
}

fn relative_to_root(root: &str, path: &str) -> Option<String> {
    if root == "." {
        Some(path.to_owned())
    } else {
        path.strip_prefix(root)?.strip_prefix('/').map(ToOwned::to_owned)
    }
}

fn target(name: Option<&str>, declared: &str, resolved_path: Option<String>) -> ManifestTarget {
    target_owned(name.map(ToOwned::to_owned), declared.to_owned(), resolved_path)
}

fn target_owned(name: Option<String>, declared: String, resolved_path: Option<String>) -> ManifestTarget {
    ManifestTarget { name: name.map(|value| bounded(&value)), declared: bounded(&declared), resolved_path }
}

fn command(kind: ManifestCommandKind, name: Option<&str>, value: &str) -> ManifestCommand {
    ManifestCommand { kind, name: name.map(bounded), command: bounded(value) }
}

fn push_target(output: &mut Vec<ManifestTarget>, target: ManifestTarget, truncated: &mut bool) {
    if output.contains(&target) {
        return;
    }
    if output.len() < MAX_ITEMS_PER_KIND {
        output.push(target);
    } else {
        *truncated = true;
    }
}

fn push_command(output: &mut Vec<ManifestCommand>, command: ManifestCommand, truncated: &mut bool) {
    if output.contains(&command) {
        return;
    }
    if output.len() < MAX_ITEMS_PER_KIND {
        output.push(command);
    } else {
        *truncated = true;
    }
}

fn normalize(metadata: &mut ManifestMetadata) {
    metadata.runtime_entry_points.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.declared.cmp(&right.declared))
    });
    metadata.library_exports.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.declared.cmp(&right.declared))
    });
    metadata.commands.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.command.cmp(&right.command))
    });
}

fn bounded(value: &str) -> String {
    value.chars().take(MAX_VALUE_CHARS).collect()
}

fn package_runner(value: &str) -> Option<&'static str> {
    match value.split_once('@').map_or(value, |(name, _)| name) {
        "npm" => Some("npm"),
        "pnpm" => Some("pnpm"),
        "yarn" => Some("yarn"),
        "bun" => Some("bun"),
        _ => None,
    }
}

fn portable_token(value: &str) -> Option<&str> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/'))
    {
        Some(value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    #[test]
    fn cargo_targets_use_declared_paths_and_bounded_commands() {
        let bytes = br#"
            [package]
            name = "custom"
            autobins = false

            [lib]
            path = "code/public.rs"

            [[bin]]
            name = "server"
            path = "code/server.rs"
        "#;
        let metadata = cargo_metadata(
            "Cargo.toml",
            bytes,
            &paths(&["Cargo.toml", "code/public.rs", "code/server.rs", "src/main.rs"]),
        )
        .expect("valid Cargo metadata");

        assert_eq!(
            metadata.library_exports[0].resolved_path.as_deref(),
            Some("code/public.rs")
        );
        assert_eq!(
            metadata.runtime_entry_points[0].resolved_path.as_deref(),
            Some("code/server.rs")
        );
        assert!(
            metadata
                .commands
                .iter()
                .any(|command| command.command == "cargo run --bin server")
        );
        assert!(
            !metadata
                .runtime_entry_points
                .iter()
                .any(|target| target.resolved_path.as_deref() == Some("src/main.rs"))
        );
    }

    #[test]
    fn package_json_collects_bins_exports_and_common_scripts() {
        let bytes = br#"{
            "name": "web",
            "bin": {"web": "./cli.js"},
            "exports": {".": "./src/index.js", "./testing": "./src/testing.js"},
            "packageManager": "pnpm@10.0.0",
            "scripts": {"build": "vite build", "test:unit": "vitest", "dev": "vite", "lint": "eslint ."}
        }"#;
        let metadata = package_json_metadata(
            "packages/web/package.json",
            bytes,
            &paths(&[
                "packages/web/package.json",
                "packages/web/cli.js",
                "packages/web/src/index.js",
                "packages/web/src/testing.js",
            ]),
        )
        .expect("valid package metadata");

        assert_eq!(
            metadata.runtime_entry_points[0].resolved_path.as_deref(),
            Some("packages/web/cli.js")
        );
        assert_eq!(metadata.library_exports.len(), 2);
        assert_eq!(metadata.commands.len(), 4);
        assert!(
            metadata
                .commands
                .iter()
                .any(|command| command.command == "pnpm run build")
        );
        assert!(
            !metadata
                .commands
                .iter()
                .any(|command| command.name.as_deref() == Some("lint"))
        );
    }

    #[test]
    fn pyproject_resolves_src_layout_modules() {
        let bytes = br#"
            [build-system]
            requires = ["hatchling"]

            [project]
            name = "sample"
            import-names = ["sample"]

            [project.scripts]
            sample = "sample.cli:main"

            [tool.pytest.ini_options]
            addopts = "-q"
        "#;
        let metadata = pyproject_metadata(
            "pyproject.toml",
            bytes,
            &paths(&["pyproject.toml", "src/sample/__init__.py", "src/sample/cli.py"]),
        )
        .expect("valid Python metadata");

        assert_eq!(
            metadata.runtime_entry_points[0].resolved_path.as_deref(),
            Some("src/sample/cli.py")
        );
        assert_eq!(
            metadata.library_exports[0].resolved_path.as_deref(),
            Some("src/sample/__init__.py")
        );
        assert!(
            metadata
                .commands
                .iter()
                .any(|command| command.command == "python -m build")
        );
        assert!(metadata.commands.iter().any(|command| command.command == "pytest"));
    }

    #[test]
    fn unsafe_and_missing_paths_remain_unresolved() {
        let known = paths(&["package.json", "src/index.js"]);
        assert_eq!(resolve_relative_path(".", "../outside.js", &known), None);
        assert_eq!(resolve_relative_path(".", "missing.js", &known), None);
    }

    #[test]
    fn manifest_item_limits_are_reported() {
        let scripts = (0..17)
            .map(|index| (format!("build:{index}"), JsonValue::String("true".to_owned())))
            .collect::<serde_json::Map<_, _>>();
        let bytes =
            serde_json::to_vec(&serde_json::json!({ "scripts": scripts })).expect("serialize bounded manifest fixture");
        let metadata =
            package_json_metadata("package.json", &bytes, &paths(&["package.json"])).expect("valid package metadata");

        assert_eq!(metadata.commands.len(), MAX_ITEMS_PER_KIND);
        assert!(metadata.truncated);
    }

    #[test]
    fn manifest_decode_failures_are_typed() {
        let known_paths = paths(&["Cargo.toml", "package.json"]);

        let utf8 = cargo_metadata("Cargo.toml", &[0xff], &known_paths).expect_err("invalid UTF-8");
        assert!(matches!(utf8, ManifestError::Utf8(_)));

        let toml = cargo_metadata("Cargo.toml", b"[", &known_paths).expect_err("invalid TOML");
        assert!(matches!(toml, ManifestError::Toml(_)));

        let json = package_json_metadata("package.json", b"{", &known_paths).expect_err("invalid JSON");
        assert!(matches!(json, ManifestError::Json(_)));

        let root = package_json_metadata("package.json", b"[]", &known_paths).expect_err("non-object JSON");
        assert!(matches!(root, ManifestError::JsonRootNotObject));
    }
}
