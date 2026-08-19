use super::*;

#[test]
fn root_map_and_history_help_are_complete() {
    let fixture = FixtureRepository::new();

    for arguments in [
        ["--help"].as_slice(),
        ["map", "--help"].as_slice(),
        ["history", "--help"].as_slice(),
    ] {
        let output = fixture.run(arguments);
        let help = stdout(&output);

        assert!(output.status.success(), "help failed: {help}");
        assert!(output.stderr.is_empty());
        assert!(help.contains("Usage:"));
        assert!(help.contains("Usage: dalil"));
        assert!(help.contains("Examples:"));
        assert!(help.contains("--format <FORMAT>"));
        assert!(help.contains("--json"));
        assert!(help.contains("--html"));
        assert!(help.contains("--open"));
        assert!(help.contains("github.com/stormlightlabs/dalil/issues"));
        if arguments.first().copied() == Some("map") {
            assert!(help.contains("--exclude <GLOB>"));
        }
    }
}

#[test]
fn default_command_returns_a_bounded_orientation_report() {
    let fixture = MixedMapFixtureRepository::new();
    let output = fixture.run(&[
        "--no-cache",
        "--focus",
        "Service",
        "--focus-path",
        "src",
        "--budget",
        "120",
        "--json",
    ]);
    let json = stdout(&output);
    let value: Value = serde_json::from_str(&json).expect("valid orientation JSON");

    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_plain_report(&json);
    assert_eq!(value["command"]["name"], "orient");
    assert_eq!(value["profile"], "compact");
    assert_eq!(value["status"], "analyzed");
    assert!(
        value["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("orientation read")
    );
    assert!(value["orientation"]["repository"]["primary_languages"].is_array());
    assert!(value["orientation"]["important_roots"].is_array());
    assert!(
        value["orientation"]["history"]
            .as_array()
            .is_some_and(|history| history.len() <= 5)
    );
    assert!(value.get("map").is_none());
    assert!(value.get("history").is_none());
    assert!(value.get("reading_plan").is_none());

    let recommendations = orientation_recommendations(&value);
    assert!(recommendations.len() <= 5, "recommendations: {recommendations:?}");
    let paths = recommendations
        .iter()
        .map(|recommendation| recommendation["path"].as_str().expect("recommendation path"))
        .collect::<BTreeSet<_>>();
    assert_eq!(paths.len(), recommendations.len(), "orientation paths must be unique");
    for recommendation in &recommendations {
        assert!(matches!(
            recommendation["purpose"].as_str(),
            Some("start_here" | "architecture" | "runtime" | "tests" | "supporting_context")
        ));
        assert!(!recommendation["reason"].as_str().unwrap_or_default().is_empty());
        assert!(!recommendation["evidence_kinds"].as_array().unwrap().is_empty());
        assert!(recommendation["confidence"].is_string());
    }
    assert!(recommendations.iter().any(|recommendation| {
        recommendation["evidence_kinds"]
            .as_array()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "focus"))
    }));
}

#[test]
fn root_and_orient_share_json_and_markdown_semantics() {
    let fixture = MixedMapFixtureRepository::new();
    let root_json = fixture.run(&["--no-cache", "--json"]);
    let orient_json = fixture.run(&["orient", "--no-cache", "--json"]);
    let root_markdown = fixture.run(&["--no-cache"]);
    let orient_markdown = fixture.run(&["orient", "--no-cache"]);

    assert!(root_json.status.success());
    assert!(orient_json.status.success());
    assert!(root_markdown.status.success());
    assert!(orient_markdown.status.success());
    assert_eq!(stdout(&root_json), stdout(&orient_json));
    assert_eq!(stdout(&root_markdown), stdout(&orient_markdown));
}

#[test]
fn default_markdown_orientation_keeps_selected_sections_readable() {
    let fixture = MixedMapFixtureRepository::new();
    let output = fixture.run(&["--no-cache"]);
    let markdown = stdout(&output);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_plain_report(&markdown);
    assert!(markdown.starts_with("# Dalil Orientation\n"));
    assert!(markdown.contains("Status: Analyzed"));
    assert!(
        markdown.chars().count().div_ceil(4) <= 1_000,
        "orientation exceeded its compact Markdown token budget"
    );
    assert!(markdown.contains("## Repository overview"));
    assert!(markdown.contains("## Start here"));
    assert!(markdown.contains("## Useful history"));
    assert!(!markdown.contains("## Source map"));
    assert!(!markdown.contains("### Ranked map selection"));
    assert!(!markdown.contains("### Churn hotspots"));
    assert!(
        markdown.lines().count() < 100,
        "orientation was {} lines",
        markdown.lines().count()
    );
    assert!(markdown.find("## Repository overview").unwrap() < markdown.find("## Start here").unwrap());
    let json = fixture.run(&["--no-cache", "--json"]);
    let json_value: Value = serde_json::from_slice(&json.stdout).expect("default orientation JSON");
    let recommendations = orientation_recommendations(&json_value);
    assert!(
        (3..=5).contains(&recommendations.len()),
        "recommendations: {recommendations:?}"
    );
    assert!(json_value.get("map").is_none());
    assert!(json_value.get("history").is_none());
    assert!(!markdown.contains("\\`, \\`"));
}

#[test]
fn lowercase_agents_documentation_is_not_an_instruction_landmark() {
    let fixture = FixtureRepository::new();
    fs::create_dir_all(fixture.root.join("docs")).expect("create documentation directory");
    write_file(
        fixture.root.join("docs/agents.md"),
        b"Documentation about agent integrations.\n",
    );

    let output = fixture.run(&["map", "--profile", "evidence", "--no-cache", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid map JSON");
    assert!(output.status.success());
    assert!(
        !value["map"]["landmarks"]
            .as_array()
            .expect("landmarks")
            .iter()
            .any(|landmark| landmark["path"] == "docs/agents.md")
    );
}

#[test]
fn default_orientation_history_is_concise_while_detailed_modes_remain_available() {
    let fixture = HistoryFixtureRepository::new();
    let concise = fixture.run(&["--no-cache"]);
    let concise_markdown = stdout(&concise);

    assert!(concise.status.success());
    assert!(concise.stderr.is_empty());
    assert!(concise_markdown.contains("## Useful history"));
    assert!(!concise_markdown.contains("### Churn hotspots"));
    assert!(!concise_markdown.contains("### Contributor concentration"));
    assert!(!concise_markdown.contains("### Monthly activity"));
    assert!(!concise_markdown.contains("#### Evidence commits"));
    let observation_count = concise_markdown.lines().filter(|line| line.starts_with("- **")).count();
    assert!(observation_count <= 5, "observations: {observation_count}");
    assert!(
        concise_markdown.find("## Repository overview").unwrap() < concise_markdown.find("## Useful history").unwrap()
    );

    let json = fixture.run(&["--no-cache", "--json"]);
    let value: Value = serde_json::from_slice(&json.stdout).expect("valid concise orientation JSON");
    let observations = value["orientation"]["history"]
        .as_array()
        .expect("history observations");
    assert!(observations.len() <= 5);
    assert!(observations.iter().all(|observation| observation["kind"].is_string()));

    let focused = fixture.run(&["--no-cache", "history"]);
    let focused_markdown = stdout(&focused);
    assert!(focused.status.success());
    assert!(focused_markdown.contains("### History observations"));
    assert!(!focused_markdown.contains("### Churn hotspots"));
    assert!(!focused_markdown.contains("### Contributor concentration"));
    assert!(!focused_markdown.contains("### Monthly activity"));
    assert!(!focused_markdown.contains("#### Evidence commits"));

    let evidence = fixture.run(&["--profile", "evidence", "--no-cache", "history"]);
    let evidence_markdown = stdout(&evidence);
    assert!(evidence.status.success());
    assert!(evidence_markdown.contains("### Churn hotspots"));
    assert!(evidence_markdown.contains("### Monthly activity"));

    let focused_evidence = fixture.run(&["--profile", "evidence", "history"]);
    let focused_evidence_markdown = stdout(&focused_evidence);
    assert!(focused_evidence.status.success());
    assert!(focused_evidence_markdown.contains("### Churn hotspots"));
    assert!(focused_evidence_markdown.contains("### Contributor concentration"));
    assert!(focused_evidence_markdown.contains("### Monthly activity"));
    assert!(focused_evidence_markdown.contains("#### Evidence commits"));
}

#[test]
fn orientation_prioritizes_root_manifest_and_conventional_entry_points() {
    let fixture = ClassificationFixtureRepository::new();
    let output = fixture.run(&["--no-cache", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid entry-point orientation JSON");
    assert!(output.status.success());

    let paths = orientation_recommendations(&value)
        .iter()
        .map(|recommendation| recommendation["path"].as_str().expect("recommendation path"))
        .collect::<Vec<_>>();
    let position = |path: &str| {
        paths
            .iter()
            .position(|candidate| *candidate == path)
            .expect("recommended path")
    };
    assert!(position("README.md") < position("Cargo.toml"), "paths: {paths:?}");
    assert!(position("Cargo.toml") < position("src/lib.rs"), "paths: {paths:?}");
    assert!(position("src/lib.rs") < position("src/main.rs"), "paths: {paths:?}");
}

#[test]
fn manifest_metadata_drives_custom_entry_points_and_common_commands() {
    let fixture = HistoryFixtureRepository::new();
    write_file(
        fixture.root.join("pyproject.toml"),
        br#"[build-system]
requires = ["hatchling"]

[project]
name = "sample"
import-names = ["sample"]

[project.scripts]
sample = "sample.cli:main"

[tool.pytest.ini_options]
addopts = "-q"
"#,
    );
    fs::create_dir_all(fixture.root.join("src/sample")).expect("create manifest metadata fixture");
    write_file(fixture.root.join("src/sample/__init__.py"), b"VALUE = 1\n");
    write_file(fixture.root.join("src/sample/cli.py"), b"def main():\n    return 0\n");

    let output = fixture.run(&["map", "--no-cache", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid manifest metadata map JSON");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let root = value["map"]["project_roots"]
        .as_array()
        .expect("project roots")
        .iter()
        .find(|root| root["path"] == ".")
        .expect("root project");
    let metadata = root["manifest_metadata"]
        .as_array()
        .expect("manifest metadata")
        .iter()
        .find(|metadata| metadata["path"] == "pyproject.toml")
        .expect("pyproject metadata");
    assert_eq!(
        metadata["runtime_entry_points"][0]["resolved_path"],
        "src/sample/cli.py"
    );
    assert!(
        metadata["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["command"] == "python -m build")
    );
    assert!(
        metadata["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["command"] == "pytest")
    );
    let orientation = fixture.run(&["orient", "--no-cache", "--json"]);
    let orientation_value: Value =
        serde_json::from_str(&stdout(&orientation)).expect("valid manifest orientation JSON");
    let runtime = orientation_value["orientation"]["runtime_entry_points"]
        .as_array()
        .expect("runtime entry points")
        .iter()
        .find(|recommendation| recommendation["purpose"] == "runtime")
        .expect("runtime recommendation");
    assert_eq!(runtime["path"], "src/sample/cli.py");
    assert!(
        runtime["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("pyproject.toml"))
    );
}

#[test]
fn compact_recovery_command_succeeds_without_including_classified_trees() {
    let fixture = MixedMapFixtureRepository::new();
    fs::create_dir_all(fixture.root.join("target/debug/deps")).expect("create ignored build tree");
    for index in 0..256 {
        write_file(
            fixture.root.join(format!("target/debug/deps/artifact-{index:03}.json")),
            b"{}\n",
        );
    }

    let compact = fixture.run(&["map", "--no-cache"]);
    let markdown = stdout(&compact);
    assert!(compact.status.success());
    assert!(!markdown.contains("target/debug/deps/artifact-"));

    let evidence = fixture.run(&["map", "--profile", "evidence", "--no-cache", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&evidence)).expect("valid evidence recovery JSON");
    assert!(
        evidence.status.success(),
        "evidence recovery failed: {}",
        String::from_utf8_lossy(&evidence.stderr)
    );
    assert_eq!(value["quality"]["resource_limited"], false);
    assert!(value["map"]["inventory"]["omitted"].as_u64().unwrap() < 64);
    assert!(
        !value["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"].as_str().is_some_and(|path| path.starts_with("target/")))
    );
}

#[test]
fn evidence_profile_is_explicit_and_reports_collection_totals() {
    let fixture = MixedMapFixtureRepository::new();
    let output = fixture.run(&["map", "--profile", "evidence", "--no-cache", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid evidence profile JSON");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(value["profile"], "evidence");
    assert_eq!(value["map"]["profile"], "evidence");
    assert!(value["map"]["collections"]["files"]["returned"].as_u64().unwrap() > 0);
    assert!(
        value["map"]["collections"]["files"]["returned"].as_u64().unwrap()
            <= value["map"]["collections"]["files"]["total"].as_u64().unwrap()
    );
}

#[test]
fn resource_bound_source_inputs_are_partial_and_typed() {
    let fixture = FixtureRepository::new();
    fs::create_dir_all(fixture.root.join("src")).expect("create source fixture directory");
    let oversized = vec![b'x'; 1_048_577];
    write_file(fixture.root.join("src/oversized.rs"), &oversized);
    write_file(fixture.root.join("src/binary.rs"), b"fn binary() {\0 }\n");

    let output = fixture.run(&["map", "--no-cache", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid bounded resource JSON");
    let omissions = value["map"]["omissions"].as_array().expect("map omissions");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(value["limits"]["max_file_bytes"], 1_048_576);
    assert!(omissions.iter().any(|omission| omission["reason"] == "oversized"));
    assert!(omissions.iter().any(|omission| omission["reason"] == "binary"));
    assert!(value["map"]["collections"]["omissions"]["total"].as_u64().unwrap() >= 2);
    assert!(stdout(&output).len() < 8 * 1_024 * 1_024);
}

#[test]
fn map_reports_bounded_landmarks_project_roots_and_recursive_boundaries() {
    let fixture = FixtureRepository::new();
    for directory in [
        "packages/app/src",
        "packages/app/tests",
        "packages/python",
        "packages/ruby",
        "packages/java",
        "packages/dotnet",
        ".github/workflows",
        "vendor/submodule",
        "nested-repo/src",
    ] {
        fs::create_dir_all(fixture.root.join(directory)).expect("create topology fixture directory");
    }
    write_file(fixture.root.join("README.md"), b"topology fixture\n");
    write_file(fixture.root.join("AGENTS.md"), b"agent instructions\n");
    write_file(fixture.root.join("CONTRIBUTING.md"), b"contributor instructions\n");
    write_file(
        fixture.root.join("Cargo.toml"),
        b"[workspace]\nmembers = [\"packages/app\"]\n",
    );
    write_file(fixture.root.join("Cargo.lock"), b"version = 3\n");
    write_file(fixture.root.join("Makefile"), b"all:\n\ttrue\n");
    write_file(fixture.root.join("CODEOWNERS"), b"* @maintainers\n");
    write_file(fixture.root.join("LICENSE"), b"license\n");
    write_file(fixture.root.join(".github/workflows/ci.yml"), b"name: CI\n");
    write_file(
        fixture.root.join(".gitmodules"),
        b"[submodule \"vendor/submodule\"]\n\tpath = vendor/submodule\n\turl = https://example.invalid/submodule\n",
    );
    write_file(fixture.root.join("packages/app/package.json"), br#"{"name":"app"}"#);
    write_file(
        fixture.root.join("packages/python/pyproject.toml"),
        b"[project]\nname = \"python\"\n",
    );
    write_file(
        fixture.root.join("packages/ruby/Gemfile"),
        b"source \"https://rubygems.org\"\n",
    );
    write_file(fixture.root.join("packages/java/pom.xml"), b"<project></project>\n");
    write_file(
        fixture.root.join("packages/dotnet/app.csproj"),
        b"<Project></Project>\n",
    );
    write_file(fixture.root.join("packages/app/src/lib.rs"), b"pub fn app() {}\n");
    write_file(
        fixture.root.join("packages/app/tests/app.rs"),
        b"#[test]\nfn app() {}\n",
    );
    write_file(
        fixture.root.join("vendor/submodule/.git"),
        b"gitdir: ../.git/modules/submodule\n",
    );
    write_file(
        fixture.root.join("nested-repo/.git"),
        b"gitdir: ../.git/worktrees/nested\n",
    );
    write_file(fixture.root.join("nested-repo/src/lib.rs"), b"pub fn nested() {}\n");
    let repository = gix::open(&fixture.root).expect("open topology fixture for tracked instruction file");
    let tree = write_tree(
        &repository,
        &[
            ("AGENTS.md", "agent instructions\n"),
            ("Cargo.toml", "[workspace]\nmembers = [\"packages/app\"]\n"),
        ],
    );
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is after the Unix epoch")
        .as_secs() as i64;
    let commit = write_commit(
        &repository,
        tree,
        &[],
        "Topology Fixture",
        "topology@example.com",
        now,
        "Topology fixture",
    );
    drop(repository);
    write_file(fixture.root.join(".git/HEAD"), b"ref: refs/heads/main\n");
    write_file(
        fixture.root.join(".git/refs/heads/main"),
        format!("{commit}\n").as_bytes(),
    );

    let output = fixture.run(&["map", "--no-cache", "--focus-path", "packages/app", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid topology JSON");
    assert!(output.status.success(), "topology map failed: {:?}", output.stderr);
    assert!(output.stderr.is_empty(), "topology stderr: {:?}", output.stderr);
    assert!(
        value["map"]["landmarks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|landmark| { landmark["kind"] == "agent_instructions" && landmark["path"] == "AGENTS.md" })
    );
    assert!(
        value["map"]["landmarks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|landmark| { landmark["kind"] == "submodule" && landmark["path"] == "vendor/submodule" })
    );
    assert!(
        value["map"]["landmarks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|landmark| { landmark["kind"] == "nested_repository" && landmark["path"] == "nested-repo" })
    );
    assert!(
        value["map"]["project_roots"]
            .as_array()
            .unwrap()
            .iter()
            .any(|root| { root["path"] == "packages/app" && root["kind"] == "package" })
    );
    assert!(
        value["map"]["project_roots"].as_array().unwrap().iter().any(|root| {
            root["path"] == "packages/app"
                && root["recommended_paths"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|path| path == "packages/app/src/lib.rs")
        }),
        "project roots: {}; landmarks: {}",
        value["map"]["project_roots"],
        value["map"]["landmarks"]
    );
    for collection in ["landmarks", "project_roots"] {
        let summary = &value["map"]["collections"][collection];
        assert!(summary["returned"].as_u64().unwrap() <= summary["total"].as_u64().unwrap());
        assert!(summary["truncated"].is_boolean());
    }
    assert!(
        !value["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| { file["path"] == "nested-repo/src/lib.rs" })
    );

    let recursive = fixture.run(&["map", "--recursive", "--no-cache", "--json"]);
    let recursive_value: Value = serde_json::from_str(&stdout(&recursive)).expect("valid recursive topology JSON");
    assert!(
        recursive.status.success(),
        "recursive map failed: {:?}",
        recursive.stderr
    );
    assert!(
        recursive_value["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| { file["path"] == "nested-repo/src/lib.rs" })
    );

    let orientation = fixture.run(&["orient", "--no-cache", "--focus-path", "packages/app", "--json"]);
    let orientation_value: Value =
        serde_json::from_slice(&orientation.stdout).expect("valid topology orientation JSON");
    let recommendations = orientation_recommendations(&orientation_value);
    assert!(recommendations.iter().any(|recommendation| {
        recommendation["project_root"] == "packages/app"
            && recommendation["evidence_kinds"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "focus"))
    }));
}
